// crates/profile/src/application/use_cases/update_display_name/update_display_name_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_display_name::UpdateDisplayNameCommand;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateDisplayNameUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateDisplayNameUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateDisplayNameCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateDisplayNameCommand) -> Result<()> {
        // 1. Récupération du profil
        let mut profile = self.repo.get_profile_without_stats(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Application du changement via le Modèle Riche
        profile.update_metadata(
            cmd.new_display_name.clone(),
            profile.bio.clone(),
            profile.location_label.clone()
        );

        // 3. Extraction des événements
        let events = profile.pull_events();

        // 4. Idempotence Applicative
        // Si l'utilisateur n'a rien changé, on s'arrête ici pour préserver les ressources.
        if events.is_empty() {
            return Ok(());
        }

        let p_cloned = profile.clone();

        // 5. Persistence Transactionnelle Atomique
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&p, Some(&mut *tx)).await?;
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}