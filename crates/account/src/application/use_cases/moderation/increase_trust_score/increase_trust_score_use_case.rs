// crates/account/src/application/increase_trust_score/increase_trust_score_use_case.rs

use shared_kernel::domain::events::{AggregateRoot, DomainEvent};
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::increase_trust_score::IncreaseTrustScoreCommand;
use crate::domain::account::entities::AccountMetadata;

pub struct IncreaseTrustScoreUseCase;

impl IncreaseTrustScoreUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, ctx: &AccountContext, cmd: IncreaseTrustScoreCommand) -> Result<AccountMetadata> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(ctx, &cmd).await
        })
        .await
    }

    async fn try_execute_once(&self, ctx: &AccountContext, cmd: &IncreaseTrustScoreCommand) -> Result<AccountMetadata> {
         let _ = ctx.ensure_id(&cmd.account_id);

        let original_metadata = ctx.metadata().await?;
        let mut metadata = original_metadata.clone();

        if !metadata.increase_trust_score(cmd.action_id, cmd.amount, &cmd.reason)?  {
            return Ok(original_metadata);
        }

        let pulled_events = metadata.pull_events();
        if pulled_events.is_empty() {
            return Ok(metadata);
        }

        let events: Vec<&dyn DomainEvent> = pulled_events.iter().map(|e| e.as_ref()).collect();
        let mut tx = ctx.begin_transaction().await?;

        ctx.save_metadata(&metadata, Some(&original_metadata), &mut *tx).await?;
        ctx.outbox_repo().save_all(&mut *tx, &events).await?;
        tx.commit().await?;


        Ok(metadata)
    }
}
