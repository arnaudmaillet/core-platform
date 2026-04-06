mod identity;
mod metadata;
mod settings;

pub use identity::AccountIdentity;
pub use metadata::AccountMetadata;
pub use settings::{AccountSettings, AccountPreferences};

#[cfg(test)]
mod tests;