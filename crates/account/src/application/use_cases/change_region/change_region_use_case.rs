// crates/account/src/application/change_region/change_region_use_case.rs

use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::use_cases::change_region::ChangeRegionCommand;
use crate::domain::entities::{Account, AccountMetadata, AccountSettings};
use crate::domain::repositories::{
    AccountMetadataRepository, AccountRepository, AccountSettingsRepository,
};

pub struct ChangeRegionResponse {
    pub account: Account,
    pub metadata: AccountMetadata,
    pub settings: AccountSettings,
}

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

    pub async fn execute(&self, command: ChangeRegionCommand) -> Result<ChangeRegionResponse> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &ChangeRegionCommand) -> Result<ChangeRegionResponse> {
        // 1. LECTURE OPTIMISTE (Hors transaction)
        let original_account = self.account_repo
            .fetch_by_id(&cmd.account_id, None).await?
            .ok_or_not_found(&cmd.account_id)?;

        let original_metadata = self.metadata_repo
            .fetch_by_account_id(&cmd.account_id).await?
            .ok_or_not_found(&cmd.account_id)?;

        let original_settings = self.settings_repo
            .fetch_by_account_id(&cmd.account_id, None).await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut account = original_account.clone();
        let mut metadata = original_metadata.clone();
        let mut settings = original_settings.clone();

        // 2. MUTATION DU MODÈLE RICHE & TEST IDEMPOTENCE
        // On vérifie si un changement est réellement nécessaire sur l'agrégat principal
        let changed_acc = account.change_region(cmd.new_region.clone())?;
        let changed_meta = metadata.change_region(cmd.new_region.clone())?;
        let changed_sett = settings.change_region(cmd.new_region.clone())?;

        if !changed_acc && !changed_meta && !changed_sett {
            return Ok(ChangeRegionResponse {
                account: original_account,
                metadata: original_metadata,
                settings: original_settings,
            });
        }

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let mut events = account.pull_events();
        events.extend(metadata.pull_events());
        events.extend(settings.pull_events());

        // 4. PRÉPARATION POUR LA TRANSACTION
        let updated_account = account.clone();
        let updated_metadata = metadata.clone();
        let updated_settings = settings.clone();
        
        let account_repo = Arc::clone(&self.account_repo);
        let metadata_repo = Arc::clone(&self.metadata_repo);
        let settings_repo = Arc::clone(&self.settings_repo);
        let outbox = Arc::clone(&self.outbox_repo);

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let account_repo = Arc::clone(&account_repo);
                let metadata_repo = Arc::clone(&metadata_repo);
                let settings_repo = Arc::clone(&settings_repo);
                let outbox = Arc::clone(&outbox);

                let u_orig = original_account.clone();
                let m_orig = original_metadata.clone();
                let s_orig = original_settings.clone();

                let u_upd = account.clone();
                let m_upd = metadata.clone();
                let s_upd = settings.clone();
                
                let events_for_tx = events.clone();

                Box::pin(async move {
                    // Sauvegarde synchronisée des 3 agrégats avec Optimistic Locking
                    account_repo.save(&u_upd, Some(&u_orig), Some(&mut *tx)).await?;
                    metadata_repo.save(&m_upd, Some(&m_orig), Some(&mut *tx)).await?;
                    settings_repo.save(&s_upd, Some(&s_orig), Some(&mut *tx)).await?;

                    // Enregistrement de tous les événements collectés
                    for event in events_for_tx {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(ChangeRegionResponse {
            account: updated_account,
            metadata: updated_metadata,
            settings: updated_settings,
        })
    }
}