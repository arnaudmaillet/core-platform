// crates/account/src/application/unsuspend_account/unsuspend_account_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::lifecycle::unsuspend::UnsuspendCommand;
use crate::domain::account::entities::AccountIdentity;
use crate::domain::repositories::AccountIdentityRepository;

pub struct UnsuspendUseCase {
    identity_repo: Arc<dyn AccountIdentityRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UnsuspendUseCase {
    pub fn new(
        identity_repo: Arc<dyn AccountIdentityRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            identity_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UnsuspendCommand) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UnsuspendCommand) -> Result<AccountIdentity> {
        let original_identity = self
            .identity_repo
            .fetch_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut identity = original_identity.clone();

        if !identity.unsuspend()? {
            return Ok(original_identity);
        }

        let events = identity.pull_events();

        if events.is_empty() {
            return Ok(identity);
        }

        let updated_identity = identity.clone();
        let identity_repo = Arc::clone(&self.identity_repo);
        let outbox_repo = Arc::clone(&self.outbox_repo);

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let identity_repo = Arc::clone(&identity_repo);
                let outbox_repo = Arc::clone(&outbox_repo);

                let original_for_tx = original_identity.clone();
                let updated_for_tx = identity.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    identity_repo.save(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx))
                        .await?;

                    for event in events_for_tx {
                        outbox_repo.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_identity)
    }
}
