use crate::domain::preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences};
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePreferencesCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub privacy: Option<PrivacyPreferences>,
    pub notifications: Option<NotificationPreferences>,
    pub appearance: Option<AppearancePreferences>,
}
