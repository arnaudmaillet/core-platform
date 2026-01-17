// crates/profile/src/application/use_cases/update_location/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{TransactionManagerExt, with_retry, RetryConfig};
use crate::application::update_location_label::UpdateLocationLabelCommand;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateLocationLabelUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateLocationLabelUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateLocationLabelCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateLocationLabelCommand) -> Result<()> {
        // 1. Récupération (Identity-only suffit pour valider la mutation)
        let mut profile = self.repo.get_profile_identity(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Application du changement
        // update_metadata encapsule l'idempotence et l'appel à apply_change()
        profile.update_metadata(
            profile.display_name.clone(),
            profile.bio.clone(),
            cmd.new_location.clone()
        );

        // 3. Extraction des événements
        let events = profile.pull_events();

        // 4. Idempotence Applicative
        // Si aucun événement n'est produit (car le label était identique), on court-circuite.
        if events.is_empty() {
            return Ok(());
        }

        let p_cloned = profile.clone();

        // 5. Persistence Transactionnelle Hyperscale
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&p, Some(&mut *tx)).await?;
                for event in events_to_process {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}