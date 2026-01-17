// crates/profile/src/application/use_cases/decrement_post_count/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{TransactionManagerExt, with_retry, RetryConfig};
use crate::application::decrement_post_count::DecrementPostCountCommand;
use crate::domain::repositories::ProfileRepository;

pub struct DecrementPostCountUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl DecrementPostCountUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: DecrementPostCountCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &DecrementPostCountCommand) -> Result<()> {
        // 1. Fetch
        let mut profile = self.repo.get_profile_identity(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Business Logic
        profile.decrement_post_count(cmd.post_id)?;

        // 3. Extraction & Clonage
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        // 5. Transaction
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_save = events;

            Box::pin(async move {
                repo.save(&p, Some(&mut *tx)).await?;
                for event in events_to_save {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }
                Ok(())
            })
        }).await?;

        Ok(())
    }
}