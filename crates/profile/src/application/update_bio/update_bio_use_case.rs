// crates/profile/src/application/use_cases/update_bio/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{TransactionManagerExt, with_retry, RetryConfig};
use crate::application::update_bio::UpdateBioCommand;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateBioUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateBioUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateBioCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateBioCommand) -> Result<()> {
        // 1. Récupération du profil
        // En Hyperscale, on ne charge que l'identité + les champs nécessaires à la mutation
        let mut profile = self.repo.get_profile_identity(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Application du changement (Modèle Riche)
        profile.update_metadata(
            profile.display_name.clone(),
            cmd.new_bio.clone(),
            profile.location_label.clone()
        );

        // 3. Extraction des événements
        let events = profile.pull_events();

        // Idempotence Applicative : Si aucun événement n'a été produit,
        // c'est que la donnée était identique. On s'arrête là (pas d'IO DB).
        if events.is_empty() {
            return Ok(());
        }

        let p_cloned = profile.clone();

        // 4. Persistence Transactionnelle
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