// crates/account/src/application/change_birth_date/change_birth_date_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::settings::change_birth_date::ChangeBirthDateCommand;
use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;

pub struct ChangeBirthDateUseCase {
    repo: Arc<dyn AccountRepository>,
    outbox: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl ChangeBirthDateUseCase {
    pub fn new(
        repo: Arc<dyn AccountRepository>,
        outbox: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: ChangeBirthDateCommand) -> Result<Account> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &ChangeBirthDateCommand) -> Result<Account> {
        // 1. Lecture Optimiste (hors transaction)
        let original_account = self
            .repo
            .fetch_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut account = original_account.clone();

        // 2. Application de la logique métier via le Modèle Riche
        if !account.change_birth_date(&cmd.region_code, cmd.birth_date.clone())? {
            return Ok(original_account);
        }

        // 3. Extraction des événements
        let events = account.pull_events();
        if events.is_empty() {
            return Ok(account);
        }

        let updated_account = account.clone();
        let repo = Arc::clone(&self.repo);
        let outbox = Arc::clone(&self.outbox);

        // 4. Persistence Transactionnelle Atomique
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);

                let original_for_tx = original_account.clone();
                let updated_for_tx = account.clone();
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

        Ok(updated_account)
    }
}
