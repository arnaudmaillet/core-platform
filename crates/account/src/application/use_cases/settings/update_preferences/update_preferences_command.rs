use crate::domain::preferences::models::{
    AppearancePreferences, NotificationPreferences, PrivacyPreferences,
};
use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_proto::account::v1::UpdatePreferencesRequest;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePreferencesCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub privacy: Option<PrivacyPreferences>,
    pub notifications: Option<NotificationPreferences>,
    pub appearance: Option<AppearancePreferences>,
}

impl UpdatePreferencesCommand {
    pub fn try_from_proto(proto: UpdatePreferencesRequest) -> Result<Self> {
        let command_id =
            Uuid::parse_str(&proto.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?;

        let account_id = AccountId::from_str(&proto.account_id)?;

        let privacy = proto.privacy.map(|p| {
            PrivacyPreferences::restore(
                p.profile_visible_to_public,
                p.show_last_active,
                p.allow_indexing,
            )
        });

        let notifications = proto.notifications.map(|n| {
            NotificationPreferences::restore(
                n.email_enabled,
                n.push_enabled,
                n.marketing_opt_in,
                n.security_alerts_only,
            )
        });

        let appearance = proto.appearance.map(|a| {
            AppearancePreferences::restore(
                // On cast l'i32 du proto vers ton enum de domaine (ex: Theme)
                a.theme.try_into().unwrap_or_default(),
                a.high_contrast,
            )
        });

        Ok(Self {
            command_id,
            account_id,
            privacy,
            notifications,
            appearance,
        })
    }
}
