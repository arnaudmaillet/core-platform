// crates/account/src/application/add_push_token/add_push_token_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::add_push_token::AddPushTokenCommand;
use crate::domain::repositories::AccountSettingsRepository;

pub struct AddPushTokenUseCase {
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl AddPushTokenUseCase {
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

    pub async fn execute(&self, command: AddPushTokenCommand) -> Result<()> {
        // En Hyperscale, les conflits de tokens sont rares mais possibles si
        // l'utilisateur se connecte sur deux devices en même temps.
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &AddPushTokenCommand) -> Result<()> {
        // 1. Récupération (Lecture hors transaction pour ne pas bloquer de lignes inutilement)
        let mut settings = self.settings_repo
            .find_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Application de la logique métier
        settings.add_push_token(cmd.token.clone())?;

        // 3. Extraction des faits
        let events = settings.pull_events();

        if events.is_empty() {
            return Ok(());
        }

        let settings_to_save = settings.clone();

        // 5. Persistance Transactionnelle Atomique
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.settings_repo.clone();
            let outbox = self.outbox_repo.clone();
            let s = settings_to_save.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&s, Some(&mut *tx)).await?;
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}