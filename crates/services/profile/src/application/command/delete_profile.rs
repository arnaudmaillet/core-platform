use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{ProfileCache, ProfileRepository};
use crate::domain::value_object::ProfileId;
use crate::error::ProfileError;

#[derive(Debug, Clone)]
pub struct DeleteProfileCommand {
    pub profile_id: String,
}

impl Command for DeleteProfileCommand {}

impl Validate for DeleteProfileCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut violations = Vec::new();
        if self.profile_id.trim().is_empty() {
            violations.push(FieldViolation::new("profile_id", "VAL-3090", "profile_id must not be empty"));
        }
        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }
}

pub struct DeleteProfileHandler {
    repo: Arc<dyn ProfileRepository>,
    cache: Arc<dyn ProfileCache>,
}

impl DeleteProfileHandler {
    pub fn new(repo: Arc<dyn ProfileRepository>, cache: Arc<dyn ProfileCache>) -> Self {
        Self { repo, cache }
    }
}

impl CommandHandler<DeleteProfileCommand> for DeleteProfileHandler {
    type Error = ProfileError;

    async fn handle(&self, envelope: Envelope<DeleteProfileCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let id = ProfileId::try_from(cmd.profile_id.as_str())?;

        let mut profile = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or_else(|| ProfileError::ProfileNotFound { id: cmd.profile_id.clone() })?;

        let handle = profile.handle().clone();
        let account_id = profile.account_id();

        profile.delete(envelope.correlation_id)?;
        self.repo.save(&profile).await?;

        // Tombstone the handle so it enters the 30-day reservation window.
        self.repo.tombstone_handle(&handle).await?;

        // Remove the lightweight account index row.
        self.repo.delete_account_index(&profile).await?;

        // Evict all affected cache namespaces.
        let _ = self.cache.invalidate_by_id(&id).await;
        let _ = self.cache.invalidate_handle(handle.as_str()).await;
        let _ = self.cache.invalidate_account_profiles(&account_id).await;

        Ok(())
    }
}
