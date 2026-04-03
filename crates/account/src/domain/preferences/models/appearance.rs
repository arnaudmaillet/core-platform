// crates/account/src/domain/preferences/models/appearance.rs

use serde::{Deserialize, Serialize};
use crate::domain::preferences::builders::AppearancePreferencesBuilder;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppearancePreferences {
    theme: ThemeMode,
    high_contrast: bool,
}

impl AppearancePreferences {
    pub fn builder() -> AppearancePreferencesBuilder {
        AppearancePreferencesBuilder::new()
    }

    pub(crate) fn restore(theme: ThemeMode, high_contrast: bool) -> Self {
        Self { theme, high_contrast }
    }
    
    pub fn theme(&self) -> ThemeMode { self.theme }
    pub fn high_contrast(&self) -> bool { self.high_contrast }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            high_contrast: false,
        }
    }
}