use shared_kernel::{
    application::BaseAppContext,
    domain::{
        events::{AggregateRoot, DomainEvent},
        repositories::{IdempotencyRepository, OutboxRepository},
        transaction::{FakeTransaction, Transaction},
        value_objects::{AccountId, RegionCode},
    },
    errors::{DomainError, Result},
    infrastructure::postgres::transactions::PostgresTransaction,
};
use std::sync::Arc;

use crate::domain::{account::entities::Account, repositories::AccountRepository};

#[derive(Clone)]
pub struct AccountAppContext {
    base: BaseAppContext,
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl AccountAppContext {
    pub fn new(
        base: BaseAppContext,
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            base,
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn create_context(&self, account_id: AccountId) -> AccountContext {
        AccountContext::new(self.clone(), account_id)
    }

    pub fn base(&self) -> &BaseAppContext {
        &self.base
    }

    pub fn account_repo(&self) -> Arc<dyn AccountRepository> {
        self.account_repo.clone()
    }

    pub fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.outbox_repo.clone()
    }

    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}

/// Le contexte d'exécution "Scoped" pour une requête unique sur un compte.
#[derive(Clone)]
pub struct AccountContext {
    app: AccountAppContext,
    account_id: AccountId,
}

impl AccountContext {
    pub(crate) fn new(app: AccountAppContext, account_id: AccountId) -> Self {
        Self { app, account_id }
    }

    // --- Accès aux Repositories ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn region(&self) -> &RegionCode {
        self.account_id.region()
    }

    pub fn account_repo(&self) -> Arc<dyn AccountRepository> {
        self.app.account_repo()
    }

    pub fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.app.outbox_repo()
    }

    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.app.base.pool()
    }

    pub fn app_ctx(&self) -> &AccountAppContext {
        &self.app
    }
    // --- Logique Métier ---

    /// Récupère l'agrégat complet.
    pub async fn account(&self) -> Result<Account> {
        let account = self
            .account_repo()
            .find_by_id(&self.account_id, None)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Account",
                id: self.account_id.to_string(),
            })?;
        Ok(account)
    }

    pub async fn save(&self, account: &mut Account, command_id: Option<uuid::Uuid>) -> Result<()> {
        // 1. Isolation du contexte
        if account.identity().account_id().uuid() != self.account_id.uuid() {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Identity mismatch: cannot change the technical UUID of an account".into(),
            });
        }
        let mut tx = self.begin_transaction().await?;

        // 2. Idempotence Technique
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

        // 3. Extraction des événements (une seule fois pour éviter de vider l'objet en cas de retry)
        let events = account.pull_events();

        // 4. Persistance (Le Repository gère l'INSERT ou l'UPDATE selon l'existence de l'ID)
        let repo_res = self.account_repo().save(account, Some(&mut *tx)).await;
        repo_res?;

        // 5. Outbox
        if !events.is_empty() {
            let event_refs: Vec<&dyn DomainEvent> = events.iter().map(|e| e.as_ref()).collect();
            self.outbox_repo().save_all(&mut *tx, &event_refs).await?;
        }

        // 6. Enregistrement de l'ID de commande (pour que le prochain appel soit bloqué)
        if let Some(cmd_id) = command_id {
            self.app.idempotency_repo().save(&mut *tx, &cmd_id).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // --- Gestion des Transactions ---

    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        match self.app.base.pool() {
            Some(pool) => {
                let tx = pool
                    .begin()
                    .await
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }
            None => {
                // En test, on tombe ici !
                Ok(Box::new(FakeTransaction::new()) as Box<dyn Transaction>)
            }
        }
    }
}
