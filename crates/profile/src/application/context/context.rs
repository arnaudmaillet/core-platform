// crates/profile/src/application/context/context.rs

use crate::{entities::Profile, repositories::ProfileRepository, types::Handle};
use infra_sqlx::{PostgresTransaction, sqlx::PgPool};
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Transaction, Versioned},
    idempotency::IdempotencyRepository,
    messaging::{Event, EventEmitter, OutboxRepository},
    types::{ProfileId, Region},
};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(any(test, feature = "test-utils"))]
use shared_kernel::core::TransactionStub;

#[derive(Clone)]
pub struct ProfileAppContext {
    pool: Option<PgPool>,
    profile_repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl ProfileAppContext {
    pub fn new(
        pool: PgPool,
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: Some(pool),
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    #[cfg(test)]
    pub fn new_stubbed(
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: None,
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn create_context(&self, profile_id: ProfileId, region: Region) -> ProfileContext {
        ProfileContext::new(self.clone(), Some(profile_id), region)
    }

    pub fn create_creation_context(&self, region: Region) -> ProfileContext {
        ProfileContext::new(self.clone(), None, region)
    }

    pub fn pg_pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
    }
    pub fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.profile_repo.clone()
    }
    pub fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.outbox_repo.clone()
    }
    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}

#[derive(Clone)]
pub struct ProfileContext {
    app: ProfileAppContext,
    profile_id: Option<ProfileId>,
    region: Region,
}

impl ProfileContext {
    pub(crate) fn new(
        app: ProfileAppContext,
        profile_id: Option<ProfileId>,
        region: Region,
    ) -> Self {
        Self {
            app,
            profile_id,
            region,
        }
    }

    pub fn region(&self) -> Region {
        self.region
    }
    pub fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.app.profile_repo()
    }

    pub fn profile_id(&self) -> Result<&ProfileId> {
        self.profile_id
            .as_ref()
            .ok_or_else(|| Error::validation("profile_id", "Profile ID missing in this context"))
    }

    // --- FLUX DE CRÉATION ---
    pub async fn ensure_creatable(
        &self,
        command_id: Uuid,
        region: &Region,
        handle: &Handle,
    ) -> Result<bool> {
        if region != &self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch for profile creation",
            ));
        }
        if !self.ensure_executable(command_id, region).await? {
            return Ok(false);
        }
        if self
            .profile_repo()
            .exists_by_handle(handle, self.region)
            .await?
        {
            return Err(Error::already_exists(
                "Profile",
                "handle",
                handle.as_str().to_string(),
            ));
        }
        if self
            .profile_repo()
            .exists_by_handle(handle, self.region)
            .await?
        {
            return Err(Error::already_exists(
                "Profile",
                "handle",
                handle.as_str().to_string(),
            ));
        }
        Ok(true)
    }

    // --- FLUX DE MODIFICATION ---
    pub async fn ensure_executable(&self, command_id: Uuid, region: &Region) -> Result<bool> {
        if region != &self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch (sharding violation prevention)",
            ));
        }
        let mut tx = self.begin_transaction().await?;
        let exists = self
            .app
            .idempotency_repo()
            .exists(Some(&mut *tx), &command_id)
            .await?;
        if exists {
            return Ok(false);
        }
        Ok(true)
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<ProfileId>) -> Result<Profile> {
        if &target.region != &self.region || Some(&target.id) != self.profile_id.as_ref() {
            return Err(Error::validation("target", "Context/Target mismatch"));
        }
        let profile = self
            .profile_repo()
            .find_by_id(target.id, self.region, None)
            .await?
            .ok_or_else(|| Error::not_found("Profile", target.id.to_string()))?;

        if profile.version() != target.expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                profile.version(),
                target.expected_version
            )));
        }
        Ok(profile)
    }

    // --- SAUVEGARDE MUTUELLE ---
    pub async fn save(&self, profile: &mut Profile, command_id: Option<Uuid>) -> Result<()> {
        if let Some(expected_id) = self.profile_id {
            if profile.profile_id() != expected_id {
                return Err(Error::validation(
                    "profile_id",
                    "Identity mismatch violation",
                ));
            }
        }

        let mut tx = self.begin_transaction().await?;

        if let Some(cmd_id) = command_id {
            if self
                .app
                .idempotency_repo()
                .exists(Some(&mut *tx), &cmd_id)
                .await?
            {
                return Err(Error::already_exists("Command", "id", cmd_id.to_string()));
            }
        }

        let events = profile.pull_events();
        self.profile_repo().save(profile, Some(&mut *tx)).await?;

        if !events.is_empty() {
            let event_refs: Vec<&dyn Event> = events.iter().map(|e| e.as_ref()).collect();
            self.app
                .outbox_repo()
                .save_all(&mut *tx, &event_refs)
                .await?;
        }

        if let Some(cmd_id) = command_id {
            self.app
                .idempotency_repo()
                .save(Some(&mut *tx), &cmd_id)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // --- GESTION DES TRANSACTIONS ---
    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        match self.app.pg_pool() {
            Some(pool) => {
                let tx = pool.begin().await.map_err(|e| {
                    Error::internal(format!("Failed to begin database transaction: {}", e))
                })?;
                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }

            // Si on est en mode test (via cargo test ou la feature de stubbing), on autorise la FakeTransaction
            #[cfg(any(test, feature = "test-utils"))]
            None => Ok(Box::new(TransactionStub::new()) as Box<dyn Transaction>),

            // En production, l'absence de pool est une erreur fatale d'initialisation
            #[cfg(not(any(test, feature = "test-utils")))]
            None => Err(Error::internal(
                "Database pool is missing. ProfileAppContext must be initialized with a valid PgPool in production.",
            )),
        }
    }
}
