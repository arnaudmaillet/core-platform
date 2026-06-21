// crates/account/src/domain/preferences/models/privacy_preferences.rs

use serde::{Deserialize, Serialize};

use crate::domain::types::PrivacyPreferencesBuilder;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacyPreferences {
    profile_visible_to_public: bool,
    show_last_active: bool,
    allow_indexing: bool,
}

impl PrivacyPreferences {
    pub fn builder() -> PrivacyPreferencesBuilder {
        PrivacyPreferencesBuilder::new()
    }

    pub(crate) fn restore(visible: bool, active: bool, index: bool) -> Self {
        let final_index = if !visible { false } else { index };

        Self {
            profile_visible_to_public: visible,
            show_last_active: active,
            allow_indexing: final_index,
        }
    }

    // Getters
    pub fn profile_visible_to_public(&self) -> bool {
        self.profile_visible_to_public
    }
    pub fn show_last_active(&self) -> bool {
        self.show_last_active
    }
    pub fn allow_indexing(&self) -> bool {
        self.allow_indexing
    }
}
