// crates/account/src/application/register_account/mod.rs

use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

use crate::application::access_management::register::RegisterCommand;
use crate::domain::account::entities::{AccountIdentity, AccountMetadata, AccountSettings};
use crate::domain::repositories::{
    AccountIdentityRepository, AccountMetadataRepository, AccountSettingsRepository,
};

pub struct RegisterUseCase {
    identity_repo: Arc<dyn AccountIdentityRepository>,
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl RegisterUseCase {
    pub fn new(
        identity_repo: Arc<dyn AccountIdentityRepository>,
        metadata_repo: Arc<dyn AccountMetadataRepository>,
        settings_repo: Arc<dyn AccountSettingsRepository>,
        outbox: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            identity_repo,
            metadata_repo,
            settings_repo,
            outbox,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: RegisterCommand) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &RegisterCommand) -> Result<AccountIdentity> {
        let account_id = AccountId::new();

        let mut identity = AccountIdentity::builder(
            account_id,
            cmd.region.clone(),
            cmd.email.clone(),
            cmd.external_id.clone(),
        )
        .with_locale(cmd.locale.clone())
        .build();

        let metadata = AccountMetadata::builder(account_id)
            .with_ip_addr(cmd.ip_addr.clone())
            .build();

        let settings = AccountSettings::builder(account_id).build();

        if !identity.register(cmd.region.clone(), cmd.ip_addr.clone())? {
            return Err(DomainError::Unexpected(
                "Account registration failed".to_string(),
            ));
        }

        let events = identity.pull_events();

        if events.is_empty() {
            return Err(DomainError::Unexpected(
                "No events generated for new account".to_string(),
            ));
        }

        let identity_repo = Arc::clone(&self.identity_repo);
        let metadata_repo = Arc::clone(&self.metadata_repo);
        let settings_repo = Arc::clone(&self.settings_repo);
        let outbox = Arc::clone(&self.outbox);
        let registered_identity = identity.clone();

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let identity = identity.clone();
                let metadata = metadata.clone();
                let settings = settings.clone();
                let external_id = cmd.external_id.clone();

                Box::pin(async move {
                    // 1. Vérification d'unicité
                    if identity_repo.exists_by_external_id(&external_id).await? {
                        return Err(DomainError::AlreadyExists {
                            entity: "Account",
                            field: "external_id",
                            value: external_id.to_string(),
                        });
                    }

                    // 2. Persistance via les repositories uniformisés
                    identity_repo.save(&identity, None, Some(&mut *tx)).await?;
                    metadata_repo.save(&metadata, None, Some(&mut *tx)).await?;
                    settings_repo.save(&settings, None, Some(&mut *tx)).await?;

                    // 3. Événements Outbox
                    for event in events {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }

                    Ok(())
                })
            })
            .await?;

        Ok(registered_identity)
    }
}
