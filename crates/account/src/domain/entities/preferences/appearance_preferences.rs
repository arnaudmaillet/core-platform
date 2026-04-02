use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppearancePreferences {
    theme: ThemeMode,
    high_contrast: bool,
}

impl AppearancePreferences {
    pub fn builder() -> AppearancePreferencesBuilder {
        AppearancePreferencesBuilder::default()
    }

    pub fn theme(&self) -> ThemeMode { self.theme }
    pub fn high_contrast(&self) -> bool { self.high_contrast }
}

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
    pub fn with_theme(mut self, theme: ThemeMode) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_high_contrast(mut self, enabled: bool) -> Self {
        self.high_contrast = enabled;
        self
    }

    pub fn build(self) -> AppearancePreferences {
        AppearancePreferences {
            theme: self.theme,
            high_contrast: self.high_contrast,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}