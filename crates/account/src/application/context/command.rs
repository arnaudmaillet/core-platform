// crates/account/src/application/context/command_context.rs

use std::sync::Arc;

use crate::application::context::AccountAppContext;
use crate::domain::entities::Account;
use crate::repositories::GlobalIdentityRegistry;
use infra_sqlx::TransactionManagerExt;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::TransactionManager;
use shared_kernel::core::{Error, Result, Versioned};
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::{AccountId, Region};
use uuid::Uuid;

pub struct AccountCommandContext<TM> {
    app: AccountAppContext<TM>,
    account_id: Option<AccountId>,
    region: Region,
}

impl<TM> Clone for AccountCommandContext<TM> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            account_id: self.account_id,
            region: self.region,
        }
    }
}

impl<TM> AccountCommandContext<TM> {
    pub(crate) fn new(
        app: AccountAppContext<TM>,
        account_id: Option<AccountId>,
        region: Region,
    ) -> Self {
        Self {
            app,
            account_id,
            region,
        }
    }

    pub fn app(&self) -> &AccountAppContext<TM> {
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

    pub fn global_registry(&self) -> Arc<dyn GlobalIdentityRegistry> {
        self.app.global_registry()
    }
}

impl<TM: TransactionManager> AccountCommandContext<TM> {
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

        let exists = self
            .app
            .transaction_manager()
            .run_in_transaction(|mut tx| async move {
                let is_present = self
                    .app
                    .idempotency_repo()
                    .exists(Some(&mut *tx), &command_id)
                    .await?;
                Ok(is_present)
            })
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

        let expected_version = target.expected_version.ok_or_else(|| {
            Error::validation(
                "expected_version",
                "Sharding strict: Expected version is missing for this transaction",
            )
        })?;

        if account.version() != expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                account.version(),
                expected_version
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

        let events = account.pull_events();

        self.app
            .transaction_manager()
            .run_in_transaction(|mut tx| async move {
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
            })
            .await?;

        Ok(())
    }
}
