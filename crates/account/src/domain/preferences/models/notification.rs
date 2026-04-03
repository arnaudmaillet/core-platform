// crates/account/src/domain/preferences/models/notification_preferences.rs

use serde::{Deserialize, Serialize};

use crate::domain::preferences::builders::NotificationPreferencesBuilder;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationPreferences {
    email_enabled: bool,
    push_enabled: bool,
    marketing_opt_in: bool,
    security_alerts_only: bool,
}

impl NotificationPreferences {
    pub fn builder() -> NotificationPreferencesBuilder {
        NotificationPreferencesBuilder::new()
    }

    pub(crate) fn restore(
        email_enabled: bool,
        push_enabled: bool,
        marketing_opt_in: bool,
        security_alerts_only: bool,
    ) -> Self {
        Self {
            email_enabled,
            push_enabled,
            marketing_opt_in,
            security_alerts_only,
        }
    }

    // Getters
    pub fn email_enabled(&self) -> bool { self.email_enabled }
    pub fn push_enabled(&self) -> bool { self.push_enabled }
    pub fn marketing_opt_in(&self) -> bool { self.marketing_opt_in }
    pub fn security_alerts_only(&self) -> bool { self.security_alerts_only }
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            email_enabled: true,
            push_enabled: true,
            marketing_opt_in: false,
            security_alerts_only: false,
        }
    }
} 