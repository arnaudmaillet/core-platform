// crates/profile/src/application/remove_banner/mod.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::remove_banner::RemoveBannerCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;

pub struct RemoveBannerUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl RemoveBannerUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: RemoveBannerCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &RemoveBannerCommand) -> Result<Profile> {
        // 1. Récupération du profil existant
        let original_profile = self
            .repo
            .assemble_full_profile(&cmd.profile_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.profile_id)?;

        // 2. Application de la suppression dans le Modèle Riche
        // Si profile.banner_url était déjà None, remove_banner() retourne false.
        let mut profile_to_update = original_profile.clone();

        if !profile_to_update.remove_banner(&cmd.region)? {
            return Ok(original_profile);
        }

        // 3. Extraction des faits (l'événement bannerRemoved est maintenant dans la liste)
        let events = profile_to_update.pull_events();

        if events.is_empty() {
            return Ok(profile_to_update);
        }

        let updated_profile = profile_to_update.clone();

        // 4. Persistence Transactionnelle
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();

                // On clone pour supporter les retries de la transaction
                let original_for_tx = original_profile.clone();
                let updated_for_tx = profile_to_update.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    // Mise à jour du profil en base (l'banner_url passera à NULL)
                    repo.save_identity(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx)).await?;

                    // Enregistrement de l'événement pour le nettoyage physique du fichier par un worker
                    for event in events_for_tx {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_profile)
    }
}
