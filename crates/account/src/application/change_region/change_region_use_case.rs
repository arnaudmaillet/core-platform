// crates/account/src/application/change_region/change_region_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;

use crate::domain::repositories::{AccountMetadataRepository, AccountSettingsRepository, AccountRepository};
use crate::application::change_region::ChangeRegionCommand;

pub struct ChangeRegionUseCase {
    account_repo: Arc<dyn AccountRepository>,
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl ChangeRegionUseCase {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        metadata_repo: Arc<dyn AccountMetadataRepository>,
        settings_repo: Arc<dyn AccountSettingsRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            account_repo,
            metadata_repo,
            settings_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: ChangeRegionCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &ChangeRegionCommand) -> Result<()> {
        // 1. RÉCUPÉRATION OPTIMISTE (Hors transaction)
        // On récupère les 3 agrégats. Note: settings n'est chargé que si nécessaire
        let mut account = self.account_repo
            .find_account_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut metadata = self.metadata_repo
            .find_by_account_id(&cmd.account_id)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut settings = self.settings_repo
            .find_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. MUTATION DES AGRÉGATS
        // Chaque entité gère son idempotence et son increment_version()
        account.change_region(cmd.new_region.clone())?;
        metadata.change_region(cmd.new_region.clone())?;

        // On met à jour la région dans les settings aussi pour la cohérence du sharding
        settings.region_code = cmd.new_region.clone();
        // On n'incrémente la version de settings que si un changement réel a eu lieu
        // (Ici on pourrait ajouter une méthode métier dans AccountSettings)

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let account_events = account.pull_events();
        let meta_events = metadata.pull_events();

        // 4. IDEMPOTENCE : Si aucune modification réelle, on arrête.
        if account_events.is_empty() && meta_events.is_empty() {
            return Ok(());
        }

        // On clone pour la transaction
        let u_to_save = account.clone();
        let m_to_save = metadata.clone();
        let s_to_save = settings.clone();

        // 5. TRANSACTION ATOMIQUE MULTI-AGRÉGATS
        self.tx_manager.run_in_transaction(move |mut tx| {
            let account_repo = self.account_repo.clone();
            let metadata_repo = self.metadata_repo.clone();
            let settings_repo = self.settings_repo.clone();
            let outbox = self.outbox_repo.clone();

            let u = u_to_save.clone();
            let m = m_to_save.clone();
            let s = s_to_save.clone();

            // Fusion des vecteurs sans clone (transfert de propriété)
            let mut events = account_events;
            events.extend(meta_events);

            Box::pin(async move {
                account_repo.save(&u, Some(&mut *tx)).await?;
                metadata_repo.save(&m, Some(&mut *tx)).await?;
                settings_repo.save(&s, Some(&mut *tx)).await?;

                for event in events {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}