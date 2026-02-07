// crates/account/src/application/increase_trust_score/increase_trust_score_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::increase_trust_score::IncreaseTrustScoreCommand;
use crate::domain::repositories::AccountMetadataRepository;

pub struct IncreaseTrustScoreUseCase {
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl IncreaseTrustScoreUseCase {
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

    pub async fn execute(&self, command: IncreaseTrustScoreCommand) -> Result<bool> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &IncreaseTrustScoreCommand) -> Result<bool> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut metadata = self
            .metadata_repo
            .find_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        if !metadata.increase_trust_score(&cmd.region_code, cmd.action_id, cmd.amount, cmd.reason.clone())?  {
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
                    // Sauvegarde avec verrouillage optimiste (OCC)
                    // Échouera si la version en DB a changé entre l'étape 1 et ici
                    repo.save(&m, Some(&mut *tx)).await?;

                    // Enregistrement des événements dans la table Outbox
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
