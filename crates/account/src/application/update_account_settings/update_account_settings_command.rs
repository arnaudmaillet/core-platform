use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use crate::domain::entities::{AppearanceSettings, NotificationSettings, PrivacySettings};

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAccountSettingsCommand {
    pub account_id: AccountId,
    pub privacy: Option<PrivacySettings>,
    pub notifications: Option<NotificationSettings>,
    pub appearance: Option<AppearanceSettings>,
}