// crates/account/src/application/update_locale/update_locale_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::update_locale::UpdateLocaleCommand;
use crate::domain::repositories::AccountRepository;

pub struct UpdateLocaleUseCase {
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateLocaleUseCase {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            account_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UpdateLocaleCommand) -> Result<bool> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpdateLocaleCommand) -> Result<bool> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut account = self
            .account_repo
            .find_account_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        if !account.update_locale(&cmd.region_code, cmd.locale.clone())? {
            return Ok(false);
        }

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = account.pull_events();
        let account_to_save = account.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.account_repo.clone();
                let outbox = self.outbox_repo.clone();
                let u = account_to_save.clone();
                let events_to_process = events;

                Box::pin(async move {
                    repo.save(&u, Some(&mut *tx)).await?;
                    for event in events_to_process {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(false)
    }
}
