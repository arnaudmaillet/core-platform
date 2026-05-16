// crates/profile/src/application/context/context.rs

use crate::{
    entities::Profile,
    repositories::ProfileRepository,
    types::{Handle, ProfileId},
};
use shared_kernel::{
    command::CommandTarget,
    context::BaseAppContext,
    core::{Error, FakeTransaction, Identifier, Result, Transaction, Versioned},
    idempotency::IdempotencyRepository,
    messaging::{Event, EventEmitter, OutboxRepository},
    postgres::PostgresTransaction,
    types::RegionCode,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ProfileAppContext {
    base: BaseAppContext,
    profile_repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl ProfileAppContext {
    pub fn new(
        base: BaseAppContext,
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            base,
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn create_context(&self, profile_id: ProfileId, region: RegionCode) -> ProfileContext {
        ProfileContext::new(self.clone(), Some(profile_id), region)
    }

    pub fn create_creation_context(&self, region: RegionCode) -> ProfileContext {
        ProfileContext::new(self.clone(), None, region)
    }

    pub fn base(&self) -> &BaseAppContext {
        &self.base
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
    profile_id: Option<ProfileId>, // Gère le cycle de vie complet (None = Création, Some = Modif)
    region: RegionCode,
}

impl ProfileContext {
    pub(crate) fn new(
        app: ProfileAppContext,
        profile_id: Option<ProfileId>,
        region: RegionCode,
    ) -> Self {
        Self {
            app,
            profile_id,
            region,
        }
    }

    pub fn region(&self) -> &RegionCode {
        &self.region
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
        region: &RegionCode,
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
            .exists_by_handle(handle, &self.region)
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
            .exists_by_handle(handle, &self.region)
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
    pub async fn ensure_executable(&self, command_id: Uuid, region: &RegionCode) -> Result<bool> {
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
            .exists(&mut *tx, &command_id)
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
            .find_by_id(&target.id, &self.region, None)
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
        if let Some(ref expected_id) = self.profile_id {
            if profile.profile_id().as_uuid() != expected_id.as_uuid() {
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
                .exists(&mut *tx, &cmd_id)
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
            self.app.idempotency_repo().save(&mut *tx, &cmd_id).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        match self.app.base.pool() {
            Some(pool) => {
                let tx = pool
                    .begin()
                    .await
                    .map_err(|e| Error::internal(e.to_string()))?;
                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }
            None => Ok(Box::new(FakeTransaction::new()) as Box<dyn Transaction>),
        }
    }
}
