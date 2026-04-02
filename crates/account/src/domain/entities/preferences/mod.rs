mod privacy_preferences;
mod notification_preferences;
mod appearance_preferences;

pub use privacy_preferences::PrivacyPreferences;
pub use notification_preferences::NotificationPreferences;
pub use appearance_preferences::{AppearancePreferences, ThemeMode};

#[cfg(test)]
mod tests;