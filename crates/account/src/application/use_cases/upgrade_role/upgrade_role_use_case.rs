// crates/account/src/application/upgrade_role/upgrade_role_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::use_cases::upgrade_role::UpgradeRoleCommand;
use crate::domain::repositories::AccountMetadataRepository;

pub struct UpgradeRoleUseCase {
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpgradeRoleUseCase {
    pub fn new(
        metadata_repo: Arc<dyn AccountMetadataRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            metadata_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UpgradeRoleCommand) -> Result<bool> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpgradeRoleCommand) -> Result<bool> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut metadata = self
            .metadata_repo
            .find_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        if !metadata.upgrade_role(&cmd.region_code, cmd.new_role.into(), cmd.reason.clone())? {
            return Ok(false);
        }


        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = metadata.pull_events();
        let metadata_cloned = metadata.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.metadata_repo.clone();
                let outbox = self.outbox_repo.clone();
                let m = metadata_cloned.clone();
                let events_to_process = events;

                Box::pin(async move {
                    repo.save(&m, Some(&mut *tx)).await?;
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
