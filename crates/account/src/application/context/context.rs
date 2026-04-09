// crates/account/src/application/context.rs

use std::sync::Arc;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::errors::{Result, DomainError};
use crate::application::context::AccountContextBuilder;
use crate::domain::account::entities::{AccountIdentity, AccountMetadata, AccountSettings};
use crate::domain::repositories::{
    AccountIdentityRepository, 
    AccountMetadataRepository, 
    AccountSettingsRepository
};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;

/// Le contexte d'exécution "Scoped" pour une requête unique.
/// Il garantit que toutes les opérations utilisent le même Shard physique.

#[derive(Clone)]
pub struct AccountContext {
    account_id: AccountId,
    region: RegionCode,
    identity_repo: Arc<dyn AccountIdentityRepository>,
    metadata_repo: Arc<dyn AccountMetadataRepository>,
    settings_repo: Arc<dyn AccountSettingsRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    pool: sqlx::PgPool,
}

impl AccountContext {
    pub(crate) fn new(
        account_id: AccountId,
        region: RegionCode,
        identity_repo: Arc<dyn AccountIdentityRepository>,
        metadata_repo: Arc<dyn AccountMetadataRepository>,
        settings_repo: Arc<dyn AccountSettingsRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        pool: sqlx::PgPool,
    ) -> Self {
        Self {
            account_id,
            region,
            identity_repo,
            metadata_repo,
            settings_repo,
            outbox_repo,
            pool,
        }
    }

    pub fn builder() -> AccountContextBuilder {
        AccountContextBuilder::new()
    }

    // --- Getters d'Accès ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn region(&self) -> &RegionCode {
        &self.region
    }

    pub fn identity_repo(&self) -> Arc<dyn AccountIdentityRepository> {
        self.identity_repo.clone()
    }

    pub fn metadata_repo(&self) -> Arc<dyn AccountMetadataRepository> {
        self.metadata_repo.clone()
    }

    pub fn settings_repo(&self) -> Arc<dyn AccountSettingsRepository> {
        self.settings_repo.clone()
    }

    pub fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.outbox_repo.clone()
    }

    // --- Gestion des Transactions ---
    // On force la transaction sur le bon shard.
    
    pub async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {        
        let tx = self.pool.begin().await
            .map_err(|e| DomainError::Internal(e.to_string()))?;
            
        Ok(Box::new(PostgresTransaction::new(tx)))
    }

    // --- High-Level Decorated API (The "Safe" Zone) ---
    
    pub async fn identity(&self) -> Result<AccountIdentity> {
        self.fetch_identity(None).await
    }

    pub async fn metadata(&self) -> Result<AccountMetadata> {
        self.fetch_metadata(None).await
    }

    pub async fn settings(&self) -> Result<AccountSettings> {
        self.fetch_settings(None).await
    }

    /// Récupère une identité avec garantie de région et gestion automatique du NotFound.
    pub async fn fetch_identity(&self, tx: Option<&mut dyn Transaction>) -> Result<AccountIdentity> {
        let account_id = &self.account_id;
        let identity = self.identity_repo
            .fetch_by_account_id(account_id, tx)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "AccountIdentity",
                id: account_id.to_string(),
            })?;
        self.ensure_region(&identity)?;

        Ok(identity)
    }

    pub async fn fetch_metadata(&self, tx: Option<&mut dyn Transaction>) -> Result<AccountMetadata> {
        let account_id = &self.account_id;
        let metadata = self.metadata_repo()
            .fetch_by_account_id(account_id, tx)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "AccountMetadata",
                id: account_id.to_string(),
            })?;
        Ok(metadata)
    }

    pub async fn fetch_settings(&self, tx: Option<&mut dyn Transaction>) -> Result<AccountSettings> {
        let account_id = &self.account_id;
        let settings = self.settings_repo()
            .fetch_by_account_id(account_id, tx)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "AccountSettings",
                id: account_id.to_string(),
            })?;
            
        Ok(settings)
    }

    /// Sauvegarde une identité en vérifiant une dernière fois la cohérence régionale.
    pub async fn save_identity(
        &self, 
        identity: &AccountIdentity, 
        original: Option<&AccountIdentity>, 
        tx: &mut dyn Transaction
    ) -> Result<()> {
        if identity.account_id() != &self.account_id {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Identity account_id mismatch for this context".into(),
            });
        }
        self.ensure_region(identity)?;
        self.identity_repo.save(identity, original, Some(tx)).await
    }

    pub async fn save_metadata(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        tx: &mut dyn Transaction
    ) -> Result<()> {
       if metadata.account_id() != &self.account_id {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Metadata account_id mismatch for this context".into(),
            });
        }
        self.metadata_repo.save(metadata, original, Some(tx)).await
    }

    /// Sauvegarde les réglages sur le shard actuel.
    pub async fn save_settings(
        &self,
        settings: &AccountSettings,
        original: Option<&AccountSettings>,
        tx: &mut dyn Transaction
    ) -> Result<()> {
        if settings.account_id() != &self.account_id {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Settings account_id mismatch for this context".into(),
            });
        }
        self.settings_repo.save(settings, original, Some(tx)).await
    }

    /// Vérifie que l'entité appartient bien à la région de ce contexte (ce shard).
    pub fn ensure_region(&self, identity: &AccountIdentity) -> Result<()> {
        if identity.region_code() != &self.region {
            return Err(DomainError::NotFound {
                entity: "AccountIdentity",
                id: identity.account_id().to_string(),
            });
        }
        Ok(())
    }

     /// Vérifie que l'id de la commande correspond bien
    pub fn ensure_id(&self, cmd_account_id: &AccountId) -> Result<()> {
        if cmd_account_id != &self.account_id {
            return Err(DomainError::Validation {
                field: "account_id".into(),
                reason: "Command account_id mismatch".into(),
            });
        }
        Ok(())
    }
}