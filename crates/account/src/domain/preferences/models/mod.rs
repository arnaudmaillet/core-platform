mod privacy;
mod notification;
mod appearance;

pub use privacy::PrivacyPreferences;
pub use notification::NotificationPreferences;
pub use appearance::{AppearancePreferences, ThemeMode};

#[cfg(test)]
mod tests;