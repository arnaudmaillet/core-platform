// crates/account/src/application/context.rs

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
    pub(crate) fn new(
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

    pub fn create_context(&self, account_id: AccountId, region: RegionCode) -> AccountContext {
        AccountContext::new(self.clone(), account_id, region)
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
    region: RegionCode,
}

impl AccountContext {
    pub(crate) fn new(app: AccountAppContext, account_id: AccountId, region: RegionCode) -> Self {
        Self {
            app,
            account_id,
            region,
        }
    }

    // --- Accès aux Repositories ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn region(&self) -> &RegionCode {
        &self.region
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

        self.ensure_region(&account)?;

        Ok(account)
    }

    /// Sauvegarde l'agrégat et ses événements dans une transaction unique.
    pub async fn save(&self, account: &mut Account, command_id: Option<uuid::Uuid>) -> Result<()> {
        // 1. Validations de sécurité
        if account.identity().account_id() != &self.account_id {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Account ID mismatch for this context".into(),
            });
        }

        // 2. Récupération des événements
        // C'est notre indicateur de changement : pas d'events = pas de modif métier
        let events = account.pull_events();

        // --- OPTIMISATION IDEMPOTENCE MÉTIER ---
        if events.is_empty() {
            if let Some(cmd_id) = command_id {
                let mut tx = self.begin_transaction().await?;

                let already_processed = self
                    .app
                    .idempotency_repo()
                    .exists(&mut *tx, &cmd_id)
                    .await?;

                if already_processed {
                    return Err(DomainError::AlreadyExists {
                        entity: "Command",
                        field: "id",
                        value: cmd_id.to_string(),
                    });
                }

                self.app.idempotency_repo().save(&mut *tx, &cmd_id).await?;
                tx.commit().await?;
            }
            return Ok(());
        }

        // 3. Persistance Atomique (si des changements existent)
        let mut tx = self.begin_transaction().await?;

        if let Some(cmd_id) = command_id {
            // A. Vérifier (Double check dans la transaction)
            let already_processed = self
                .app
                .idempotency_repo()
                .exists(&mut *tx, &cmd_id)
                .await?;
            if already_processed {
                return Err(DomainError::AlreadyExists {
                    entity: "Command",
                    field: "id",
                    value: cmd_id.to_string(),
                });
            }
        }

        // B. Sauvegarde de l'agrégat (la version monte ici via le domaine)
        self.account_repo().save(account, Some(&mut *tx)).await?;

        // C. Outbox
        let event_refs: Vec<&dyn DomainEvent> = events.iter().map(|e| e.as_ref()).collect();
        self.outbox_repo().save_all(&mut *tx, &event_refs).await?;

        // D. Marquer la commande
        if let Some(cmd_id) = command_id {
            self.app.idempotency_repo().save(&mut *tx, &cmd_id).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // --- Gestion des Transactions ---

    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        // On utilise l'accesseur du base_ctx qui renvoie maintenant Option<&PgPool>
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

    // --- Sécurité ---

    fn ensure_region(&self, account: &Account) -> Result<()> {
        if account.identity().region_code() != &self.region {
            return Err(DomainError::NotFound {
                entity: "Account",
                id: account.identity().account_id().to_string(),
            });
        }
        Ok(())
    }
}
