// crates/account/src/application/remove_push_token/remove_push_token_use_case.rs

use crate::application::remove_token::RemovePushTokenCommand;
use crate::domain::repositories::AccountSettingsRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct RemovePushTokenUseCase {
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl RemovePushTokenUseCase {
    pub fn new(
        settings_repo: Arc<dyn AccountSettingsRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            settings_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: RemovePushTokenCommand) -> Result<bool> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &RemovePushTokenCommand) -> Result<bool> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut settings = self
            .settings_repo
            .find_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        if !settings.remove_push_token(&cmd.region_code, &cmd.token)? {
            return Ok(false);
        }


        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = settings.pull_events();
        let settings_to_save = settings.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.settings_repo.clone();
                let outbox = self.outbox_repo.clone();
                let s = settings_to_save.clone();
                let events_to_process = events;

                Box::pin(async move {
                    repo.save(&s, Some(&mut *tx)).await?;
                    for event in events_to_process {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(true)
    }
}
