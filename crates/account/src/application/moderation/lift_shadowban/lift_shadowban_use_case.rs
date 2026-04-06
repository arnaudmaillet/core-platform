// crates/account/src/application/lift_shadowban/lift_shadowban_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::moderation::lift_shadowban::LiftShadowbanCommand;
use crate::domain::account::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;

pub struct LiftShadowbanUseCase {
    repo: Arc<dyn AccountMetadataRepository>,
    outbox: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl LiftShadowbanUseCase {
    pub fn new(
        repo: Arc<dyn AccountMetadataRepository>,
        outbox: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: LiftShadowbanCommand) -> Result<AccountMetadata> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &LiftShadowbanCommand) -> Result<AccountMetadata> {
        let original_metadata = self
            .repo
            .fetch_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut metadata = original_metadata.clone();

        if !metadata.lift_shadowban(&cmd.region_code, cmd.reason.clone())? {
            return Ok(original_metadata);
        }

        let events = metadata.pull_events();

        let updated_metadata = metadata.clone();
        let repo = Arc::clone(&self.repo);
        let outbox = Arc::clone(&self.outbox);

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);
                
                let original_for_tx = original_metadata.clone();
                let updated_for_tx = metadata.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    repo.save(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx)).await?;

                    for event in events_for_tx {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_metadata)
    }
}
