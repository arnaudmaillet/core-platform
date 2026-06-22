use chrono::{DateTime, Utc};

use crate::domain::event::reaction_event::{ReactionRemovedEvent, ReactionUpsertedEvent};
use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;

/// The Reaction aggregate encapsulates the invariant:
/// a profile may hold exactly one active reaction per post at any point in time.
///
/// All mutation methods return the domain event that describes the state transition.
/// The events are published to Kafka by the command handler (not stored in this struct).
pub struct Reaction {
    post_id:    PostId,
    profile_id: ProfileId,
    kind:       ReactionKind,
    weight:     i64,
    reacted_at: DateTime<Utc>,
}

impl Reaction {
    /// Reconstitutes an aggregate from ScyllaDB ledger state.
    pub fn reconstitute(
        post_id:    PostId,
        profile_id: ProfileId,
        kind:       ReactionKind,
        weight:     i64,
        reacted_at: DateTime<Utc>,
    ) -> Self {
        Self { post_id, profile_id, kind, weight, reacted_at }
    }

    /// Creates the domain event for a reaction swap (or first reaction).
    ///
    /// `old_reaction` is `Some` when a previous Redis state was swapped out by
    /// the Lua script. Its presence drives the counter decrement on the
    /// write-behind path.
    pub fn build_upserted_event(
        post_id:      &PostId,
        profile_id:   &ProfileId,
        new_kind:     ReactionKind,
        new_weight:   i64,
        old_reaction: Option<(ReactionKind, i64)>,
    ) -> ReactionUpsertedEvent {
        let (old_kind, old_weight) = old_reaction
            .map(|(k, w)| (Some(k), Some(w)))
            .unwrap_or((None, None));

        ReactionUpsertedEvent {
            post_id:     post_id.as_str(),
            profile_id:  profile_id.as_str(),
            new_kind,
            new_weight,
            old_kind,
            old_weight,
            event_at_ms: Utc::now().timestamp_millis(),
        }
    }

    /// Creates the domain event for a reaction removal.
    pub fn build_removed_event(
        post_id:    &PostId,
        profile_id: &ProfileId,
        kind:       ReactionKind,
        weight:     i64,
    ) -> ReactionRemovedEvent {
        ReactionRemovedEvent {
            post_id:     post_id.as_str(),
            profile_id:  profile_id.as_str(),
            kind,
            weight,
            event_at_ms: Utc::now().timestamp_millis(),
        }
    }

    pub fn post_id(&self)    -> &PostId    { &self.post_id }
    pub fn profile_id(&self) -> &ProfileId { &self.profile_id }
    pub fn kind(&self)       -> ReactionKind { self.kind }
    pub fn weight(&self)     -> i64 { self.weight }
    pub fn reacted_at(&self) -> DateTime<Utc> { self.reacted_at }
}

fn _assert_send_sync() {
    fn _check<T: Send + Sync>() {}
    _check::<EngagementError>();
}
