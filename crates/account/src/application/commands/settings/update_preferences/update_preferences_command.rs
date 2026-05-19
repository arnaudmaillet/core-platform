use crate::types::{AppearancePreferences, NotificationPreferences, PrivacyPreferences};
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{AccountId, Region};
use shared_proto::account::v1::UpdatePreferencesRequest;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePreferencesCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub privacy: Option<PrivacyPreferences>,
    pub notifications: Option<NotificationPreferences>,
    pub appearance: Option<AppearancePreferences>,
}

impl IdentifiableCommand for UpdatePreferencesCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl UpdatePreferencesCommand {
    pub fn try_from_proto(req: UpdatePreferencesRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        let privacy = req.privacy.map(|p| {
            PrivacyPreferences::restore(
                p.profile_visible_to_public,
                p.show_last_active,
                p.allow_indexing,
            )
        });

        let notifications = req.notifications.map(|n| {
            NotificationPreferences::restore(
                n.email_enabled,
                n.push_enabled,
                n.marketing_opt_in,
                n.security_alerts_only,
            )
        });

        let appearance = req.appearance.map(|a| {
            AppearancePreferences::restore(
                // On cast l'i32 du proto vers ton enum de domaine (ex: Theme)
                a.theme.try_into().unwrap_or_default(),
                a.high_contrast,
            )
        });

        Ok(Self {
            command_id,
            target,
            privacy,
            notifications,
            appearance,
        })
    }
}
