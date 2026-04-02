use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacyPreferences {
    profile_visible_to_public: bool,
    show_last_active: bool,
    allow_indexing: bool,
}

impl PrivacyPreferences {
    pub fn builder() -> PrivacyPreferencesBuilder {
        PrivacyPreferencesBuilder::default()
    }

    // Getters
    pub fn profile_visible_to_public(&self) -> bool { self.profile_visible_to_public }
    pub fn show_last_active(&self) -> bool { self.show_last_active }
    pub fn allow_indexing(&self) -> bool { self.allow_indexing }
}

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
        let mut prefs = PrivacyPreferences {
            profile_visible_to_public: self.profile_visible_to_public,
            show_last_active: self.show_last_active,
            allow_indexing: self.allow_indexing,
        };

        // Règle métier : Si le profil n'est pas public, on force l'interdiction d'indexation
        if !prefs.profile_visible_to_public {
            prefs.allow_indexing = false;
        }

        prefs
    }
}