// crates/account/src/application/update_timezone/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{with_retry, RetryConfig, TransactionManagerExt};
use crate::application::update_timezone::update_timezone_command::UpdateTimezoneCommand;
use crate::domain::repositories::AccountSettingsRepository;

pub struct UpdateAccountTimezoneUseCase {
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateAccountTimezoneUseCase {
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

    pub async fn execute(&self, command: UpdateTimezoneCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateTimezoneCommand) -> Result<()> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut settings = self.settings_repo
            .find_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        settings.update_timezone(cmd.new_timezone.clone())?;

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = settings.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        // On évite de solliciter PostgreSQL si l'état n'a pas changé.
        if events.is_empty() {
            return Ok(());
        }

        let settings_to_save = settings.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.settings_repo.clone();
            let outbox = self.outbox_repo.clone();
            let s = settings_to_save.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&s, Some(&mut *tx)).await?;
                for event in events_to_process {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}