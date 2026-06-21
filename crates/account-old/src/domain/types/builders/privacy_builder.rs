// crates/account/src/domain/preferences/builders/privacy_builder.rs

use crate::domain::types::PrivacyPreferences;

pub struct PrivacyPreferencesBuilder {
    profile_visible_to_public: bool,
    show_last_active: bool,
    allow_indexing: bool,
}

impl Default for PrivacyPreferencesBuilder {
    fn default() -> Self {
        Self {
            profile_visible_to_public: true,
            show_last_active: true,
            allow_indexing: true,
        }
    }
}

impl PrivacyPreferencesBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_public_profile(mut self, visible: bool) -> Self {
        self.profile_visible_to_public = visible;
        self
    }

    pub fn with_last_active(mut self, show: bool) -> Self {
        self.show_last_active = show;
        self
    }

    pub fn with_indexing(mut self, allow: bool) -> Self {
        self.allow_indexing = allow;
        self
    }

    pub fn build(self) -> PrivacyPreferences {
        PrivacyPreferences::restore(
            self.profile_visible_to_public,
            self.show_last_active,
            self.allow_indexing,
        )
    }
}
