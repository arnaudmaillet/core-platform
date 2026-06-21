// crates/account/src/domain/preferences/builders/notification_builder.rs

use crate::types::NotificationPreferences;

pub struct NotificationPreferencesBuilder {
    email_enabled: bool,
    push_enabled: bool,
    marketing_opt_in: bool,
    security_alerts_only: bool,
}

impl Default for NotificationPreferencesBuilder {
    fn default() -> Self {
        Self {
            email_enabled: true,
            push_enabled: true,
            marketing_opt_in: false,
            security_alerts_only: false,
        }
    }
}

impl NotificationPreferencesBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_email(mut self, enabled: bool) -> Self {
        self.email_enabled = enabled;
        self
    }

    pub fn with_push(mut self, enabled: bool) -> Self {
        self.push_enabled = enabled;
        self
    }

    pub fn with_marketing(mut self, opt_in: bool) -> Self {
        self.marketing_opt_in = opt_in;
        self
    }

    pub fn with_security_only(mut self, security_only: bool) -> Self {
        self.security_alerts_only = security_only;
        self
    }

    pub fn build(self) -> NotificationPreferences {
        NotificationPreferences::restore(
            self.email_enabled,
            self.push_enabled,
            self.marketing_opt_in,
            self.security_alerts_only,
        )
    }
}
