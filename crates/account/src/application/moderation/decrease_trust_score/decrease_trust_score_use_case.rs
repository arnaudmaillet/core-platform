// crates/account/src/application/decrease_trust_score/decrease_trust_score_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::moderation::decrease_trust_score::DecreaseTrustScoreCommand;
use crate::domain::account::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;

pub struct DecreaseTrustScoreUseCase {
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl DecreaseTrustScoreUseCase {
    pub fn new(
        metadata_repo: Arc<dyn AccountMetadataRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            metadata_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: DecreaseTrustScoreCommand) ->Result<AccountMetadata> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &DecreaseTrustScoreCommand) -> Result<AccountMetadata> {
        let original_metadata = self
            .metadata_repo
            .fetch_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;
        
        let mut metadata = original_metadata.clone();
        
        if !metadata.decrease_trust_score(cmd.action_id, cmd.amount, cmd.reason.clone())? {
            return Ok(original_metadata);
        }

         let events = metadata.pull_events();

        let updated_metadata = metadata.clone();
        let metadata_repo = Arc::clone(&self.metadata_repo);
        let outbox_repo = Arc::clone(&self.outbox_repo);

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let metadata_repo = Arc::clone(&metadata_repo);
                let outbox_repo = Arc::clone(&outbox_repo);

                let original_for_tx = original_metadata.clone();
                let updated_for_tx = metadata.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    metadata_repo.save(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx)).await?;

                    for event in events_for_tx {
                        outbox_repo.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_metadata)
    }
}
