// crates/profile/src/application/use_cases/decrement_post_count/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::decrement_post_count::DecrementPostCountCommand;
use crate::domain::entities::Profile;
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

    pub async fn execute(&self, command: DecrementPostCountCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &DecrementPostCountCommand) -> Result<Profile> {
        // 1. Fetch
        let mut profile = self.repo.get_profile_without_stats(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Business Logic
        profile.decrement_post_count(cmd.post_id);

        // 3. Extraction & Clonage
        let events = profile.pull_events();

        if events.is_empty() {
            return Ok(profile);
        }

        let updated_profile = profile.clone();

        // 5. Transaction
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let profile = profile.clone();
            let events = events.clone();

            Box::pin(async move {
                repo.save(&profile, Some(&mut *tx)).await?;
                for event in events {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }
                Ok(())
            })
        }).await?;

        Ok(updated_profile)
    }
}