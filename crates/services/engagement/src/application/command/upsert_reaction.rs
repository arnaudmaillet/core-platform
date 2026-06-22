use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EngagementEventPublisher, ScoreStore};
use crate::config::ReactionWeightsConfig;
use crate::domain::aggregate::Reaction;
use crate::domain::event::reaction_event::ReactionKafkaEvent;
use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;

pub struct UpsertReactionCommand {
    pub post_id:    String,
    pub profile_id: String,
    /// Proto ReactionKind ordinal (1-based).
    pub kind:       i32,
}

impl Command for UpsertReactionCommand {}

impl Validate for UpsertReactionCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "ENG-VAL-001", "post_id must not be empty"));
        }
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "ENG-VAL-002", "profile_id must not be empty"));
        }
        if self.kind < 1 || self.kind > 5 {
            v.push(FieldViolation::new("kind", "ENG-VAL-003", "reaction kind must be between 1 and 5"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct UpsertReactionHandler<S, P> {
    pub score_store: Arc<S>,
    pub publisher:   Arc<P>,
    pub weights:     Arc<ReactionWeightsConfig>,
}

impl<S, P> CommandHandler<UpsertReactionCommand> for UpsertReactionHandler<S, P>
where
    S: ScoreStore,
    P: EngagementEventPublisher,
{
    type Error = EngagementError;

    async fn handle(&self, envelope: Envelope<UpsertReactionCommand>) -> Result<(), EngagementError> {
        let cmd = &envelope.payload;

        let post_id    = PostId::try_from(cmd.post_id.as_str())?;
        let profile_id = ProfileId::try_from(cmd.profile_id.as_str())?;
        let kind       = ReactionKind::from_proto(cmd.kind)?;
        let weight     = self.weights.weight_of(kind);

        // Hot path: single Redis round-trip (Lua script). No ScyllaDB touch.
        let old_reaction = self.score_store
            .atomic_upsert_reaction(&post_id, &profile_id, kind, weight)
            .await?;

        let event = Reaction::build_upserted_event(&post_id, &profile_id, kind, weight, old_reaction);

        // Kafka publish is the write-behind trigger for ScyllaDB durability.
        self.publisher
            .publish_reaction_event(&ReactionKafkaEvent::Upserted(event))
            .await?;

        tracing::debug!(
            post_id    = %post_id,
            profile_id = %profile_id,
            kind       = kind.as_redis_key(),
            weight,
            "reaction upserted"
        );

        Ok(())
    }
}
