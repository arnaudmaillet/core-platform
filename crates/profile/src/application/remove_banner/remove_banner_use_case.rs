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
        let mut profile = self
            .repo
            .get_profile_by_account_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Application de la suppression dans le Modèle Riche
        // Si profile.banner_url était déjà None, remove_banner() retourne false.
        if !profile.remove_banner(&cmd.region)? {
            return Ok(profile);
        }

        // 3. Extraction des faits (l'événement bannerRemoved est maintenant dans la liste)
        let events = profile.pull_events();

        if events.is_empty() {
            return Ok(profile);
        }

        let updated_profile = profile.clone();

        // 4. Persistence Transactionnelle
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();

                // On clone pour supporter les retries de la transaction
                let profile = profile.clone();
                let events = events.clone();

                Box::pin(async move {
                    // Mise à jour du profil en base (l'banner_url passera à NULL)
                    repo.save(&profile, Some(&mut *tx)).await?;

                    // Enregistrement de l'événement pour le nettoyage physique du fichier par un worker
                    for event in events {
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
