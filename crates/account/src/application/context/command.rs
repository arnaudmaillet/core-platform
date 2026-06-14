// crates/account/src/application/context/command_context.rs

use std::sync::Arc;

use crate::application::context::AccountKernelCtx;
use crate::domain::entities::Account;
use crate::repositories::GlobalIdentityRegistry;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::TransactionManagerExt;
use shared_kernel::core::{Error, Result, Versioned};
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::{AccountId, Region};
use uuid::Uuid;

#[derive(Clone)]
pub struct AccountCommandCtx {
    kernel: AccountKernelCtx,
    account_id: Option<AccountId>,
    region_cmd: Region,
}

impl AccountCommandCtx {
    pub(crate) fn new(
        kernel: AccountKernelCtx,
        account_id: Option<AccountId>,
        region_cmd: Region,
    ) -> Self {
        Self {
            kernel,
            account_id,
            region_cmd,
        }
    }

    pub fn app(&self) -> &AccountKernelCtx {
        &self.kernel
    }

    pub fn region(&self) -> Region {
        self.region_cmd
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
        self.kernel.global_registry()
    }

    pub fn verify_actors(&self, account_id: AccountId) -> Result<()> {
        if let Some(expected_id) = self.account_id {
            if account_id != expected_id {
                return Err(Error::validation(
                    "target",
                    "Context/Target identity mismatch",
                ));
            }
        }
        Ok(())
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<AccountId>) -> Result<Account> {
        if self.region_cmd != self.kernel.cluster_region() {
            return Err(Error::validation(
                "region",
                format!(
                    "Sharding violation prevention: Command region '{}' mismatch with deployment cluster region '{}'",
                    self.region_cmd,
                    self.kernel.cluster_region()
                ),
            ));
        }

        self.verify_actors(target.id)?;

        let account = self
            .kernel
            .account_repo()
            .find_by_id(self.region_cmd, target.id, None)
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

    pub async fn save(&self, account: &mut Account, command_id: Uuid) -> Result<()> {
        self.verify_actors(account.account_id())?;

        let events = account.pull_events();
        let account_repo = self.kernel.account_repo();
        let outbox_repo = self.kernel.outbox_repo();
        let idempotency_repo = self.kernel.idempotency_repo();
        let region = self.region_cmd;

        self.kernel
            .transaction_manager()
            .run_transaction(move |tx| {
                Box::pin(async move {
                    let already_processed = idempotency_repo.exists(Some(tx), &command_id).await?;
                    if already_processed {
                        tracing::warn!(
                            command_id = %command_id,
                            "Idempotence DB : Commande déjà appliquée dans ce Shard. Skip."
                        );
                        return Ok(());
                    }

                    account_repo.save(region, account, Some(tx)).await?;
                    if !events.is_empty() {
                        let event_refs: Vec<&dyn Event> =
                            events.iter().map(|e| e.as_ref()).collect();
                        outbox_repo.save_all(region, tx, &event_refs).await?;
                    }

                    idempotency_repo.save(Some(tx), &command_id).await?;

                    Ok(())
                })
            })
            .await?;

        Ok(())
    }
}
