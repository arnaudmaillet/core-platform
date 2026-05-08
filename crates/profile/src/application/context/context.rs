// crates/profile/src/application/context.rs

use crate::domain::{entities::Profile, repositories::ProfileRepository, value_objects::ProfileId};
use shared_kernel::{
    application::BaseAppContext,
    domain::{
        Identifier, events::{DomainEvent, EventEmitter}, repositories::{IdempotencyRepository, OutboxRepository}, transaction::{FakeTransaction, Transaction}, value_objects::RegionCode
    },
    errors::{DomainError, Result},
    infrastructure::postgres::transactions::PostgresTransaction,
};
use std::sync::Arc;

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
        ProfileContext::new(self.clone(), profile_id, region)
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
    profile_id: ProfileId,
    region: RegionCode,
}

impl ProfileContext {
    pub(crate) fn new(app: ProfileAppContext, profile_id: ProfileId, region: RegionCode) -> Self {
        Self {
            app,
            profile_id,
            region,
        }
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn region(&self) -> &RegionCode {
        &self.region
    }
    pub fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.app.profile_repo()
    }
    pub fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.app.outbox_repo()
    }

    /// Récupère l'agrégat Profile depuis le Repo
    pub async fn profile(&self) -> Result<Profile> {
        self.profile_repo()
            .fetch(&self.profile_id, &self.region)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Profile",
                id: self.profile_id.to_string(),
            })
    }

    /// Sauvegarde atomique : Persistance + Outbox + Idempotence
    pub async fn save(&self, profile: &mut Profile, command_id: Option<uuid::Uuid>) -> Result<()> {
        // 1. Garde-fou technique : l'ID ne doit pas avoir changé
        if profile.profile_id().as_uuid() != self.profile_id.as_uuid() {
            return Err(DomainError::Validation {
                field: "profile_id".into(),
                reason: "Identity mismatch: cannot change the technical UUID of a profile".into(),
            });
        }

        let mut tx = self.begin_transaction().await?;

        // 2. Idempotence
        if let Some(cmd_id) = command_id {
            if self
                .app
                .idempotency_repo()
                .exists(&mut *tx, &cmd_id)
                .await?
            {
                return Err(DomainError::AlreadyExists {
                    entity: "Command",
                    field: "id".into(),
                    value: cmd_id.to_string(),
                });
            }
        }

        // 3. Cycle de vie des événements
        let events = profile.pull_events();

        // 4. Persistance (Repository gère l'OCC avec la version)
        self.profile_repo().save(profile, Some(&mut *tx)).await?;

        // 5. Outbox pattern
        if !events.is_empty() {
            let event_refs: Vec<&dyn DomainEvent> = events.iter().map(|e| e.as_ref()).collect();
            self.app
                .outbox_repo()
                .save_all(&mut *tx, &event_refs)
                .await?;
        }

        // 6. Enregistrement Idempotence
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
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }
            None => Ok(Box::new(FakeTransaction::new()) as Box<dyn Transaction>),
        }
    }
}
