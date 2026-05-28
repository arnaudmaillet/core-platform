use crate::application::context::AccountAppContext;
use crate::domain::entities::Account;
use infra_sqlx::PostgresTransaction;
use shared_kernel::messaging::EventEmitter;
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Transaction, Versioned},
    messaging::Event,
    types::{AccountId, Region},
};
use uuid::Uuid;

#[cfg(any(test, feature = "test-utils"))]
use shared_kernel::core::TransactionStub;

#[derive(Clone)]
pub struct AccountCommandContext {
    app: AccountAppContext,
    account_id: Option<AccountId>,
    region: Region,
}

impl AccountCommandContext {
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

    pub fn app(&self) -> &AccountAppContext {
        &self.app
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn account_id(&self) -> Result<&AccountId> {
        self.account_id.as_ref().ok_or_else(|| {
            Error::validation(
                "account_id",
                "Account ID is missing in this context (Creation flow)",
            )
        })
    }

    pub async fn ensure_creatable(&self, command_id: Uuid, command_region: Region) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch for account creation",
            ));
        }
        self.ensure_executable(command_id, command_region).await
    }

    pub async fn ensure_executable(
        &self,
        command_id: Uuid,
        command_region: Region,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                format!(
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

        Ok(!exists)
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<AccountId>) -> Result<Account> {
        if target.region != self.region || Some(&target.id) != self.account_id.as_ref() {
            return Err(Error::validation(
                "target",
                "Context/Target identity or sharding mismatch",
            ));
        }

        let account = self
            .app
            .account_repo()
            .find_by_id(self.region, target.id, None)
            .await?
            .ok_or_else(|| Error::not_found("Account", target.id.to_string()))?;

        if account.version() != target.expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                account.version(),
                target.expected_version
            )));
        }

        Ok(account)
    }

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

        let events = account.pull_events();

        if !events.is_empty() {
            self.app
                .account_repo()
                .save(self.region, account, Some(&mut *tx))
                .await?;

            let event_refs: Vec<&dyn Event> = events.iter().map(|e| e.as_ref()).collect();
            self.app
                .outbox_repo()
                .save_all(self.region, &mut *tx, &event_refs)
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

    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        match self.app.pg_pool() {
            Some(pool) => {
                let tx = pool
                    .begin()
                    .await
                    .map_err(|e| Error::internal(e.to_string()))?;
                Ok(Box::new(PostgresTransaction::new(tx)) as Box<dyn Transaction>)
            }
            #[cfg(any(test, feature = "test-utils"))]
            None => Ok(Box::new(TransactionStub::new()) as Box<dyn Transaction>),

            #[cfg(not(any(test, feature = "test-utils")))]
            None => Err(Error::internal(
                "Database pool is missing. AccountAppContext must be initialized with a valid PgPool in production.",
            )),
        }
    }
}
