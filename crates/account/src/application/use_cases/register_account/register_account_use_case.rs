// crates/account/src/application/register_account/mod.rs

use chrono::Utc;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::use_cases::register_account::RegisterAccountCommand;
use crate::domain::entities::{Account, AccountMetadata, AccountSettings};
use crate::domain::events::AccountEvent;
use crate::domain::repositories::{
    AccountMetadataRepository, AccountRepository, AccountSettingsRepository,
};

pub struct RegisterAccountUseCase {
    account_repo: Arc<dyn AccountRepository>,
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl RegisterAccountUseCase {
    // pub async fn execute(&self, command: RegisterAccountCommand) -> Result<AccountId> {
    //     with_retry(RetryConfig::default(), || async {
    //         self.try_execute_once(&command).await
    //     }).await
    // }
    //
    // async fn try_execute_once(&self, cmd: &RegisterAccountCommand) -> Result<AccountId> {
    //     // --- ÉTAPE 1 : Build full account entity ---
    //     let account_id = AccountId::new();
    //     let account_id_for_return = account_id.clone();
    //
    //     let account = Account::builder(account_id.clone(), cmd.region.clone(), cmd.username.clone(), cmd.email.clone(), cmd.external_id.clone())
    //         .with_locale(cmd.locale.clone())
    //         .build();
    //
    //     let mut metadata_builder = AccountMetadata::builder(account_id.clone(), cmd.region.clone());
    //     if let Some(ip) = cmd.ip_address.clone() {
    //         metadata_builder = metadata_builder.with_estimated_ip(ip);
    //     }
    //     let metadata = metadata_builder.build();
    //
    //     let settings = AccountSettings::builder(account_id.clone(), cmd.region.clone())
    //         .build();
    //
    //     // --- ÉTAPE 2 : Exécution (Dans la Transaction) ---
    //     self.tx_manager.run_in_transaction(|mut tx| {
    //         // Clones nécessaires pour le déplacement dans la closure async move
    //         let account_repo = self.account_repo.clone();
    //         let metadata_repo = self.metadata_repo.clone();
    //         let settings_repo = self.settings_repo.clone();
    //         let outbox = self.outbox_repo.clone();
    //
    //         let account = account.clone();
    //         let metadata = metadata.clone();
    //         let settings = settings.clone();
    //         let external_id = cmd.external_id.clone();
    //         let events = account_repo.pull_events();
    //         let events_to_save = events;
    //
    //         Box::pin(async move {
    //             // 1. Vérification d'unicité
    //             if account_repo.find_account_id_by_external_id(&external_id, Some(&mut *tx)).await?.is_some() {
    //                 return Err(DomainError::AlreadyExists {
    //                     entity: "Account",
    //                     field: "external_id",
    //                     value: external_id.as_str().to_string(),
    //                 });
    //             }
    //
    //             // 2. Persistance via les repositories uniformisés
    //             account_repo.save(&account, Some(&mut *tx)).await?;
    //             metadata_repo.save(&metadata, Some(&mut *tx)).await?;
    //             settings_repo.save(&settings, Some(&mut *tx)).await?;
    //
    //             // 3. Événements Outbox
    //             for event in events_to_save {
    //                 outbox.save(&mut *tx, event.as_ref()).await?;
    //             }
    //
    //             Ok(())
    //         })
    //     }).await?;
    //
    //     // --- ÉTAPE 3 : Succès ---
    //     // Si on arrive ici, la transaction est commitée.
    //     Ok(account_id_for_return)
    // }
}
