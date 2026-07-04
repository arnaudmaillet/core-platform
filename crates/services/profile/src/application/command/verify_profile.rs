use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, ProfileCache, ProfileRepository};
use crate::domain::value_object::{ProfileId, VerificationKind};
use crate::error::ProfileError;

#[derive(Debug, Clone)]
pub struct VerifyProfileCommand {
    pub profile_id: String,
    pub verification_kind: String,
}

impl Command for VerifyProfileCommand {}

impl Validate for VerifyProfileCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut violations = Vec::new();
        if self.profile_id.trim().is_empty() {
            violations.push(FieldViolation::new("profile_id", "VAL-3060", "profile_id must not be empty"));
        }
        if self.verification_kind.trim().is_empty() {
            violations.push(FieldViolation::new("verification_kind", "VAL-3061", "verification_kind must not be empty"));
        }
        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }
}

pub struct VerifyProfileHandler {
    repo: Arc<dyn ProfileRepository>,
    cache: Arc<dyn ProfileCache>,
    publisher: Arc<dyn EventPublisher>,
}

impl VerifyProfileHandler {
    pub fn new(repo: Arc<dyn ProfileRepository>, cache: Arc<dyn ProfileCache>, publisher: Arc<dyn EventPublisher>) -> Self {
        Self { repo, cache, publisher }
    }
}

impl CommandHandler<VerifyProfileCommand> for VerifyProfileHandler {
    type Error = ProfileError;

    async fn handle(&self, envelope: Envelope<VerifyProfileCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let id = ProfileId::try_from(cmd.profile_id.as_str())?;
        let kind = VerificationKind::try_from(cmd.verification_kind.as_str())?;

        let mut profile = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or_else(|| ProfileError::ProfileNotFound { id: cmd.profile_id.clone() })?;

        profile.verify(kind, envelope.correlation_id)?;
        self.repo.save(&profile).await?;

        for event in profile.drain_events() {
            self.publisher.publish(&event).await?;
        }
        let _ = self.cache.invalidate_by_id(&id).await;

        Ok(())
    }
}
