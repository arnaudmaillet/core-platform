use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationPreferences {
    email_enabled: bool,
    push_enabled: bool,
    marketing_opt_in: bool,
    security_alerts_only: bool,
}

impl NotificationPreferences {
    pub fn builder() -> NotificationPreferencesBuilder {
        NotificationPreferencesBuilder::default()
    }

    // Getters
    pub fn email_enabled(&self) -> bool { self.email_enabled }
    pub fn push_enabled(&self) -> bool { self.push_enabled }
    pub fn marketing_opt_in(&self) -> bool { self.marketing_opt_in }
    pub fn security_alerts_only(&self) -> bool { self.security_alerts_only }
}

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
        NotificationPreferences {
            email_enabled: self.email_enabled,
            push_enabled: self.push_enabled,
            marketing_opt_in: self.marketing_opt_in,
            security_alerts_only: self.security_alerts_only,
        }
    }
}