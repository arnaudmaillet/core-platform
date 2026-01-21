// crates/profile/src/application/use_cases/update_media/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_media::UpdateMediaCommand;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateMediaUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateMediaUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateMediaCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateMediaCommand) -> Result<()> {
        // 1. Récupération du profil
        let mut profile = self.repo.get_profile_without_stats(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Application du changement via le Modèle Riche
        // Ici, on passe les deux URLs. Si l'une est None, elle sera "supprimée"
        if let Some(new_avatar_option) = &cmd.avatar_url {
            if !profile.update_avatar(new_avatar_option.clone()){
                return Ok(())
            };
        }

        // Si l'utilisateur a envoyé une info pour la bannière
        if let Some(new_banner_option) = &cmd.banner_url {
            if !profile.update_banner(new_banner_option.clone()){
                return Ok(())
            };
        }

        // 3. Extraction des événements (MediaUpdated généré ici)
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        // 4. Persistence atomique
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events; // Move direct, pas de clone

            Box::pin(async move {
                // Sauvegarde de l'état (avec vérification de version)
                repo.save(&p, Some(&mut *tx)).await?;

                // Envoi vers l'Outbox pour déclencher les effets de bord (CDN/Cleanup)
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}