// crates/account/src/application/lift_shadowban/lift_shadowban_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::lift_shadowban::LiftShadowbanCommand;
use crate::domain::repositories::AccountMetadataRepository;

pub struct LiftShadowbanUseCase {
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl LiftShadowbanUseCase {
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

    pub async fn execute(&self, command: LiftShadowbanCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &LiftShadowbanCommand) -> Result<()> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut metadata = self
            .metadata_repo
            .find_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        if metadata.region_code() != &cmd.region_code {
            return Err(DomainError::Validation {
                field: "region_code",
                reason: "This account does not belong to the specified region".into(),
            });
        }

        // 2. MUTATION DU MODÈLE RICHE
        metadata.lift_shadowban(cmd.reason.clone());

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = metadata.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        // Si l'utilisateur n'était pas shadowbanned, aucun événement n'est produit.
        if events.is_empty() {
            return Ok(());
        }

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

        Ok(())
    }
}
