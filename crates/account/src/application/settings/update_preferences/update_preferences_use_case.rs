// crates/account/src/application/update_settings/mod.rs

use crate::application::settings::update_preferences::UpdatePreferencesCommand;
use crate::domain::account::entities::AccountSettings;
use crate::domain::repositories::AccountSettingsRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct UpdatePreferencesUseCase {
    repo: Arc<dyn AccountSettingsRepository>,
    outbox: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdatePreferencesUseCase {
    pub fn new(
        repo: Arc<dyn AccountSettingsRepository>,
        outbox: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UpdatePreferencesCommand) -> Result<AccountSettings> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpdatePreferencesCommand) -> Result<AccountSettings> {
        let original_settings = self
            .repo
            .fetch_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut settings = original_settings.clone();
        let mut changed = false;

        if let Some(privacy) = &cmd.privacy {
            changed |= settings.update_privacy_preferences(&cmd.region_code, privacy.clone())?;
        }

        if let Some(notifications) = &cmd.notifications {
            changed |= settings
                .update_notifications_preferences(&cmd.region_code, notifications.clone())?;
        }

        if let Some(appearance) = &cmd.appearance {
            changed |=
                settings.update_appearance_preferences(&cmd.region_code, appearance.clone())?;
        }

        if !changed {
            return Ok(original_settings);
        }

        let events = settings.pull_events();

        let updated_settings = settings.clone();
        let repo = Arc::clone(&self.repo);
        let outbox = Arc::clone(&self.outbox);

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);

                let original_for_tx = original_settings.clone();
                let updated_for_tx = settings.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    repo.save(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx))
                        .await?;

                    for event in events_for_tx {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_settings)
    }
}
