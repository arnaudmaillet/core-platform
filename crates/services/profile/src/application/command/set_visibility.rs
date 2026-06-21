use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{ProfileCache, ProfileRepository};
use crate::domain::value_object::{ProfileId, ProfileVisibility};
use crate::error::ProfileError;

#[derive(Debug, Clone)]
pub struct SetVisibilityCommand {
    pub profile_id: String,
    pub visibility: String,
}

impl Command for SetVisibilityCommand {}

impl Validate for SetVisibilityCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut violations = Vec::new();
        if self.profile_id.trim().is_empty() {
            violations.push(FieldViolation::new("profile_id", "VAL-3050", "profile_id must not be empty"));
        }
        if self.visibility.trim().is_empty() {
            violations.push(FieldViolation::new("visibility", "VAL-3051", "visibility must not be empty"));
        }
        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }
}

pub struct SetVisibilityHandler {
    repo: Arc<dyn ProfileRepository>,
    cache: Arc<dyn ProfileCache>,
}

impl SetVisibilityHandler {
    pub fn new(repo: Arc<dyn ProfileRepository>, cache: Arc<dyn ProfileCache>) -> Self {
        Self { repo, cache }
    }
}

impl CommandHandler<SetVisibilityCommand> for SetVisibilityHandler {
    type Error = ProfileError;

    async fn handle(&self, envelope: Envelope<SetVisibilityCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let id = ProfileId::try_from(cmd.profile_id.as_str())?;
        let visibility = ProfileVisibility::try_from(cmd.visibility.as_str())?;

        let mut profile = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or_else(|| ProfileError::ProfileNotFound { id: cmd.profile_id.clone() })?;

        profile.set_visibility(visibility, envelope.correlation_id)?;
        self.repo.save(&profile).await?;
        let _ = self.cache.invalidate_by_id(&id).await;
        let _ = self.cache.invalidate_account_profiles(&profile.account_id()).await;

        Ok(())
    }
}
