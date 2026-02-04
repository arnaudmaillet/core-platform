// crates/account/src/domain/repositories/account_settings_repository.rs

use crate::domain::entities::AccountSettings;
use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::domain::value_objects::{PushToken, Timezone};
use shared_kernel::errors::Result;

#[async_trait]
pub trait AccountSettingsRepository: Send + Sync {
    /// Récupère les réglages d'un utilisateur.
    /// Retourne `None` si l'utilisateur n'a pas encore de réglages personnalisés (on utilisera alors les Defaults).
    async fn find_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountSettings>>;

    /// Sauvegarde ou met à jour les réglages (Upsert).
    /// En Hyperscale, on utilise souvent l'Atomic Upsert pour éviter les Race Conditions
    /// entre deux mises à jour simultanées (ex: Mobile + Web).
    async fn save(
        &self,
        settings: &AccountSettings,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    /// Met à jour uniquement la timezone (optimisation pour les changements de région).
    async fn update_timezone(
        &self,
        account_id: &AccountId,
        timezone: &Timezone,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    /// Ajoute un push token de manière atomique (évite de charger/sauver tout l'objet).
    /// Crucial pour la performance quand un utilisateur a plusieurs devices.
    async fn add_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    /// Supprime un push token spécifique (ex: lors d'un logout ou token expiré).
    async fn remove_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    /// Supprime tous les réglages (généralement lors de la suppression définitive du compte).
    async fn delete_for_user(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
