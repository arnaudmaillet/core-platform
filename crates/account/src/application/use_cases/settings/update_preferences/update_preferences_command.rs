use crate::domain::preferences::models::{
    AppearancePreferences, NotificationPreferences, PrivacyPreferences,
};
use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePreferencesCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub privacy: Option<PrivacyPreferences>,
    pub notifications: Option<NotificationPreferences>,
    pub appearance: Option<AppearancePreferences>,
}
