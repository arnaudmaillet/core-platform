// crates/account/src/application/set_beta_status/set_as_beta_account_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;

use crate::domain::repositories::AccountMetadataRepository;
use crate::application::set_as_beta_account::SetAsBetaAccountCommand;

pub struct SetAsBetaAccountUseCase {
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl SetAsBetaAccountUseCase {
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

    pub async fn execute(&self, command: SetAsBetaAccountCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &SetAsBetaAccountCommand) -> Result<()> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let mut metadata = self.metadata_repo
            .find_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. MUTATION DU MODÈLE RICHE
        // L'entité vérifie si le statut change réellement.
        // Si oui : metadata.metadata.increment_version() + Event "BetaStatusChanged"
        metadata.set_beta_status(cmd.status, cmd.reason.clone());

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = metadata.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        // Si le statut est déjà identique, aucun événement n'est produit.
        if events.is_empty() {
            return Ok(());
        }

        let metadata_cloned = metadata.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.metadata_repo.clone();
            let outbox = self.outbox_repo.clone();
            let m = metadata_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                // Sauvegarde avec verrouillage optimiste (OCC)
                // Échouera si la version en DB ne correspond plus (collision)
                repo.save(&m, Some(&mut *tx)).await?;

                // Patterns Outbox pour notifier les services de Feature Flagging
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}