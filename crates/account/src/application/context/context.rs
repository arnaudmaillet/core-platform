// crates/account/src/application/context/context.rs

use crate::domain::{entities::Account, repositories::AccountRepository};
use infra_sqlx::{PostgresTransaction, sqlx::PgPool};
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Transaction, Versioned},
    idempotency::IdempotencyRepository,
    messaging::{Event, EventEmitter, OutboxRepository},
    types::{AccountId, Region},
};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(any(test, feature = "test-utils"))]
use shared_kernel::core::FakeTransaction;

#[derive(Clone)]
pub struct AccountAppContext {
    pool: Option<PgPool>,
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl AccountAppContext {
    pub fn new(
        pool: PgPool,
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: Some(pool),
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    #[cfg(test)]
    pub fn new_stubbed(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: None,
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    /// Crée un contexte pour la modification ou la lecture : la région est extraite automatiquement de l'ID autoportant.
    pub fn create_context(&self, account_id: AccountId) -> AccountContext {
        let region = account_id.region();
        AccountContext::new(self.clone(), Some(account_id), region)
    }

    /// Crée un contexte pour la création : l'ID n'existe pas encore, on passe la région cible pour router la DB.
    pub fn create_creation_context(&self, region: Region) -> AccountContext {
        AccountContext::new(self.clone(), None, region)
    }

    pub fn pg_pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
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

/// Le contexte d'exécution "Scoped" unifié pour le domaine Account (Unit of Work)
#[derive(Clone)]
pub struct AccountContext {
    app: AccountAppContext,
    account_id: Option<AccountId>,
    region: Region,
}

impl AccountContext {
    pub(crate) fn new(
        app: AccountAppContext,
        account_id: Option<AccountId>,
        region: Region,
    ) -> Self {
        Self {
            app,
            account_id,
            region,
        }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn account_repo(&self) -> Arc<dyn AccountRepository> {
        self.app.account_repo()
    }

    pub fn account_id(&self) -> Result<&AccountId> {
        self.account_id.as_ref().ok_or_else(|| {
            Error::validation("account_id", "Account ID missing in this execution context")
        })
    }

    // --- FLUX DE CRÉATION ---
    /// Valide l'idempotence technique en amont de la création de l'agrégat.
    pub async fn ensure_creatable(&self, command_id: Uuid) -> Result<bool> {
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

    // --- FLUX DE MODIFICATION / VALIDATION IDEMPOTENCE ---
    /// Valide l'idempotence technique et la cohérence géographique d'une commande sur un agrégat existant.
    pub async fn ensure_executable(
        &self,
        command_id: Uuid,
        command_region: Region,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                &format!(
                    "Sharding violation prevention: Command region '{}' mismatch with context region '{}'",
                    command_region, self.region
                ),
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

    // --- LECTURE SÉCURISÉE (OCC & SHARDING) ---
    /// Charge un compte et valide son intégrité territoriale ainsi que sa concurrence optimiste (OCC).
    pub async fn fetch_verified(&self, target: &CommandTarget<AccountId>) -> Result<Account> {
        // Validation immédiate via l'ID autoportant : pas de désalignement possible
        if target.id.region() != self.region || Some(&target.id) != self.account_id.as_ref() {
            return Err(Error::validation(
                "target",
                "Context/Target identity or sharding mismatch",
            ));
        }

        let account = self
            .account_repo()
            .find_by_id(target.id, None)
            .await?
            .ok_or_else(|| Error::not_found("Account", target.id.to_string()))?;

        // Double sécurité anti-corruption de données
        if account.identity().region() != self.region {
            return Err(Error::internal(format!(
                "Data Integrity Violation: Account {} belongs to region {}, but context is sharded on {}",
                target.id,
                account.identity().region(),
                self.region
            )));
        }

        // Contrôle de concurrence optimiste (OCC)
        if account.version() != target.expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                account.version(),
                target.expected_version
            )));
        }

        Ok(account)
    }

    // --- SAUVEGARDE ATOMIQUE (OUTBOX + IDEMPOTENCE) ---
    /// Persiste l'agrégat, extrait ses événements pour l'outbox et valide définitivement l'idempotence dans une seule transaction.
    pub async fn save(&self, account: &mut Account, command_id: Option<Uuid>) -> Result<()> {
        if let Some(expected_id) = self.account_id {
            if account.account_id() != expected_id {
                return Err(Error::validation(
                    "account_id",
                    "Identity mismatch violation",
                ));
            }
        }

        let mut tx = self.begin_transaction().await?;

        // SÉCURITÉ CONCURRENCE : Lock d'idempotence strict avant toute écriture métier
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

        // 1. Extraction des événements produits par l'OperationTracker
        let events = account.pull_events();

        // 2. 💡 ÉCRITURE CONDITIONNELLE SÉCURISÉE
        if !events.is_empty() {
            self.account_repo().save(account, Some(&mut *tx)).await?;

            let event_refs: Vec<&dyn Event> = events.iter().map(|e| e.as_ref()).collect();
            self.app
                .outbox_repo()
                .save_all(&mut *tx, &event_refs)
                .await?;
        } else {
            tracing::debug!(account_id = %account.account_id(), "Idempotence métier : écriture du compte court-circuitée");
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

    pub fn update_expected_identity(&mut self, new_account_id: AccountId) {
        self.account_id = Some(new_account_id);
        // Note: On ne change pas self.region ici car le commit de suppression/écriture
        // se fait encore sur le Shard / Pool de connexion d'origine.
    }

    //ERREURRR
    // --- GESTION DES TRANSACTIONS ---
    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        match self.app.pg_pool() {
            Some(pool) => {
                let tx = pool
                    .begin()
                    .await
                    .map_err(|e| Error::internal(e.to_string()))?;
                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }
            // Si on est en mode test (via cargo test ou la feature de stubbing), on autorise la FakeTransaction
            #[cfg(any(test, feature = "test-utils"))]
            None => Ok(Box::new(FakeTransaction::new()) as Box<dyn Transaction>),

            // En production, l'absence de pool est une erreur fatale d'initialisation
            #[cfg(not(any(test, feature = "test-utils")))]
            None => Err(Error::internal(
                "Database pool is missing. AccountAppContext must be initialized with a valid PgPool in production.",
            )),
        }
    }
}
