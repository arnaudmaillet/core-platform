use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, ProfileCache, ProfileRepository};
use crate::domain::value_object::ProfileId;
use crate::error::ProfileError;

/// Denormalize an author tier onto a profile. Driven by the `social-graph`
/// author-tier consumer; idempotent (an unchanged tier is a no-op).
#[derive(Debug, Clone)]
pub struct SetProfileTierCommand {
    pub profile_id: String,
    /// 0=Standard, 1=Premium, 2=Vip.
    pub tier: u8,
}

impl Command for SetProfileTierCommand {}

impl Validate for SetProfileTierCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut violations = Vec::new();
        if self.profile_id.trim().is_empty() {
            violations.push(FieldViolation::new("profile_id", "VAL-3070", "profile_id must not be empty"));
        }
        if self.tier > 2 {
            violations.push(FieldViolation::new("tier", "VAL-3071", "tier must be 0, 1, or 2"));
        }
        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }
}

pub struct SetProfileTierHandler {
    repo: Arc<dyn ProfileRepository>,
    cache: Arc<dyn ProfileCache>,
    publisher: Arc<dyn EventPublisher>,
}

impl SetProfileTierHandler {
    pub fn new(repo: Arc<dyn ProfileRepository>, cache: Arc<dyn ProfileCache>, publisher: Arc<dyn EventPublisher>) -> Self {
        Self { repo, cache, publisher }
    }
}

impl CommandHandler<SetProfileTierCommand> for SetProfileTierHandler {
    type Error = ProfileError;

    async fn handle(&self, envelope: Envelope<SetProfileTierCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let id = ProfileId::try_from(cmd.profile_id.as_str())?;

        let mut profile = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or_else(|| ProfileError::ProfileNotFound { id: cmd.profile_id.clone() })?;

        profile.set_tier(cmd.tier, envelope.correlation_id)?;

        // Idempotent: an unchanged tier produces no event (and no version bump), so
        // there is nothing to persist — skip the LWT UPDATE entirely.
        let events = profile.drain_events();
        if events.is_empty() {
            return Ok(());
        }

        self.repo.save(&profile).await?;
        for event in &events {
            self.publisher.publish(event).await?;
        }
        let _ = self.cache.invalidate_by_id(&id).await;

        Ok(())
    }
}
