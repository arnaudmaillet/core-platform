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
use crate::domain::account::entities::AccountIdentity;
use crate::domain::repositories::AccountIdentityRepository;

pub struct ChangeBirthDateUseCase {
    identity_repo: Arc<dyn AccountIdentityRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl ChangeBirthDateUseCase {
    pub fn new(
        identity_repo: Arc<dyn AccountIdentityRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            identity_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: ChangeBirthDateCommand) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &ChangeBirthDateCommand) -> Result<AccountIdentity> {
        // 1. Lecture Optimiste (hors transaction)
        let original_identity = self
            .identity_repo
            .fetch_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut identity = original_identity.clone();

        // 2. Application de la logique métier via le Modèle Riche
        if !identity.change_birth_date(cmd.birth_date.clone())? {
            return Ok(original_identity);
        }

        // 3. Extraction des événements
        let events = identity.pull_events();
        if events.is_empty() {
            return Ok(identity);
        }

        let updated_identity = identity.clone();
        let repo = Arc::clone(&self.identity_repo);
        let outbox = Arc::clone(&self.outbox_repo);

        // 4. Persistence Transactionnelle Atomique
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);

                let original_for_tx = original_identity.clone();
                let updated_for_tx = identity.clone();
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

        Ok(updated_identity)
    }
}
