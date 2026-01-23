// crates/account/src/application/change_email/change_email_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::errors::Result;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;

use crate::application::change_email::ChangeEmailCommand;
use crate::domain::repositories::AccountRepository;

pub struct ChangeEmailUseCase {
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl ChangeEmailUseCase {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { account_repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: ChangeEmailCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &ChangeEmailCommand) -> Result<()> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut account = self.account_repo
            .find_account_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        account.change_email(cmd.new_email.clone())?;

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = account.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        if events.is_empty() {
            return Ok(());
        }

        let account_cloned = account.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.account_repo.clone();
            let outbox = self.outbox_repo.clone();
            let u = account_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                // Sauvegarde avec vérification de version (Optimistic Lock)
                repo.save(&u, Some(&mut *tx)).await?;

                // Enregistrement des événements (EmailChanged, etc.)
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}