use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_location::update_location_command::UpdateLocationCommand;
use crate::domain::repositories::LocationRepository;

pub struct UpdateLocationUseCase {
    repo: Arc<dyn LocationRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateLocationUseCase {
    pub fn new(
        repo: Arc<dyn LocationRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateLocationCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateLocationCommand) -> Result<()> {
        // 1. Récupération
        let mut location = self.repo.find_by_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Throttling Métier (Optimisation de la charge DB)
        // On ne fait rien si le mouvement est insignifiant.
        let distance_moved = location.coordinates().distance_to(&cmd.coords);
        let time_since_last_update = chrono::Utc::now() - location.updated_at();

        if distance_moved < 5.0 && time_since_last_update.num_seconds() < 30 {
            return Ok(());
        }

        // 3. Mutation de l'Agrégat
        location.update_position(
            cmd.coords.clone(),
            cmd.metrics.clone(),
            cmd.movement.clone()
        );

        // 4. Extraction & Clonage
        let events = location.pull_events();

        // Idempotence : Si aucun événement (ex: validation interne échouée), on sort.
        if events.is_empty() {
            return Ok(());
        }

        let loc_cloned = location.clone();

        // 5. Persistance Transactionnelle (Atomique)
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let l = loc_cloned.clone();
            let evs = events;

            Box::pin(async move {
                // Save avec Optimistic Locking (WHERE version = current_version)
                repo.save(&l, Some(&mut *tx)).await?;

                // Enregistrement des événements dans la table Outbox
                for event in evs {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}