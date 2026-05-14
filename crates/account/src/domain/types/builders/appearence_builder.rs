// crates/account/src/domain/preferences/builders/appearence_builder.rs

use crate::types::{AppearancePreferences, ThemeMode};

pub struct AppearancePreferencesBuilder {
    theme: ThemeMode,
    high_contrast: bool,
}

impl Default for AppearancePreferencesBuilder {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            high_contrast: false,
        }
    }
}

impl AppearancePreferencesBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_theme(mut self, theme: ThemeMode) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_high_contrast(mut self, enabled: bool) -> Self {
        self.high_contrast = enabled;
        self
    }

    pub fn build(self) -> AppearancePreferences {
        AppearancePreferences::restore(self.theme, self.high_contrast)
    }
}