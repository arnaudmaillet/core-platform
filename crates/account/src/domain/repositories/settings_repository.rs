// crates/account/src/domain/repositories/account_settings_repository.rs

use crate::domain::account::entities::AccountSettings;
use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::domain::value_objects::{PushToken, Timezone};
use shared_kernel::errors::Result;

#[async_trait]
pub trait AccountSettingsRepository: Send + Sync {
    async fn fetch_by_account_id(
        &self,
        account_id: &AccountId,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountSettings>>;

    async fn save(
        &self,
        settings: &AccountSettings,
        original: Option<&AccountSettings>,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
