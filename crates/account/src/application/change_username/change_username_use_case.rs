// crates/account/src/application/change_email/change_username_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{with_retry, RetryConfig, TransactionManagerExt};

use crate::application::change_username::ChangeUsernameCommand;
use crate::domain::repositories::AccountRepository;

pub struct ChangeUsernameUseCase {
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl ChangeUsernameUseCase {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { account_repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: ChangeUsernameCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &ChangeUsernameCommand) -> Result<()> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut user = self.account_repo
            .find_account_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        // L'entité vérifie si le username change et appelle apply_change()
        user.change_username(cmd.new_username.clone())?;

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = user.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        if events.is_empty() {
            return Ok(());
        }

        let user_cloned = user.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.account_repo.clone();
            let outbox = self.outbox_repo.clone();
            let u = user_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                // Sauvegarde avec Optimistic Locking (WHERE version = current)
                // Note : le repo lèvera une erreur DomainError::AlreadyExists si le
                // nouveau username est déjà pris (via contrainte UNIQUE DB).
                repo.save(&u, Some(&mut *tx)).await?;

                // Enregistrement des événements (UsernameChanged)
                for event in events_to_process {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}