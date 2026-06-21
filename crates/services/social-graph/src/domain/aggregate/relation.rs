use chrono::{DateTime, Utc};

use crate::domain::event::{
    DomainEvent, ProfileBlocked, ProfileFollowed, ProfileUnblocked, ProfileUnfollowed,
};
use crate::domain::value_object::{ProfileId, RelationStatus};
use crate::error::SocialGraphError;

/// The timestamps of follow edges severed by a block operation.
///
/// Returned by [`Relation::block`] so the command handler knows exactly which
/// ScyllaDB DELETEs and Redis SREMs to issue without re-querying the database.
#[derive(Debug, Clone)]
pub struct SeveredFollows {
    /// `Some(ts)` if the actor→target follow was severed; `ts` is `followed_at`
    /// needed to delete the clustering row from `followers` and `following`.
    pub actor_to_target: Option<DateTime<Utc>>,
    /// `Some(ts)` if the target→actor follow was severed.
    pub target_to_actor: Option<DateTime<Utc>>,
}

/// The raw bidirectional context used to reconstruct a [`Relation`].
///
/// Populated by [`SocialGraphRepository::load_relation`].
pub struct RelationContext {
    pub actor_follows_target_since:  Option<DateTime<Utc>>,
    pub target_follows_actor_since:  Option<DateTime<Utc>>,
    pub actor_blocks_target:         bool,
    pub target_blocks_actor:         bool,
}

/// Aggregate root for the bidirectional relationship between two profiles.
///
/// Enforces all social-graph invariants before any persistence call is made:
///   1. Self-interaction guard (actor == target is rejected at the handler level).
///   2. Block-gate: a follow is rejected if a block exists in either direction.
///   3. Idempotency guards: re-follow and re-block return domain errors.
///   4. Block-sever: `block()` automatically computes which existing follows
///      must be deleted, returning their timestamps as [`SeveredFollows`].
///
/// # Event sourcing
///
/// Domain events are accumulated in `pending_events` and drained by the command
/// handler after persistence succeeds. The handler owns the publish-or-discard
/// decision; the aggregate never publishes directly.
pub struct Relation {
    actor_id:  ProfileId,
    target_id: ProfileId,

    /// `None` = actor does not follow target.
    /// `Some(ts)` = actor follows target since `ts` (needed for DELETE key).
    actor_follows_target_since: Option<DateTime<Utc>>,

    /// `None` = target does not follow actor.
    /// `Some(ts)` = target follows actor since `ts`.
    target_follows_actor_since: Option<DateTime<Utc>>,

    actor_blocks_target: bool,
    target_blocks_actor: bool,

    pending_events: Vec<DomainEvent>,
}

impl Relation {
    pub fn from_context(
        actor_id:  ProfileId,
        target_id: ProfileId,
        ctx: RelationContext,
    ) -> Self {
        Self {
            actor_id,
            target_id,
            actor_follows_target_since: ctx.actor_follows_target_since,
            target_follows_actor_since: ctx.target_follows_actor_since,
            actor_blocks_target:        ctx.actor_blocks_target,
            target_blocks_actor:        ctx.target_blocks_actor,
            pending_events:             Vec::new(),
        }
    }

    // ── Commands ──────────────────────────────────────────────────────────────

    /// Records that the actor follows the target.
    ///
    /// # Errors
    ///
    /// - [`SocialGraphError::AlreadyFollowing`] if the follow already exists.
    /// - [`SocialGraphError::BlockGateDenied`] if a block exists in either direction.
    pub fn follow(&mut self) -> Result<DateTime<Utc>, SocialGraphError> {
        if self.actor_blocks_target || self.target_blocks_actor {
            return Err(SocialGraphError::BlockGateDenied {
                actor_id:  self.actor_id.as_str(),
                target_id: self.target_id.as_str(),
            });
        }
        if self.actor_follows_target_since.is_some() {
            return Err(SocialGraphError::AlreadyFollowing {
                actor_id:  self.actor_id.as_str(),
                target_id: self.target_id.as_str(),
            });
        }
        let now = Utc::now();
        self.actor_follows_target_since = Some(now);
        self.pending_events.push(DomainEvent::ProfileFollowed(ProfileFollowed {
            actor_id:    self.actor_id,
            target_id:   self.target_id,
            followed_at: now,
        }));
        Ok(now)
    }

    /// Removes the actor→target follow.
    ///
    /// # Errors
    ///
    /// - [`SocialGraphError::NotFollowing`] if no follow exists.
    pub fn unfollow(&mut self) -> Result<DateTime<Utc>, SocialGraphError> {
        let followed_at = self.actor_follows_target_since.ok_or_else(|| {
            SocialGraphError::NotFollowing {
                actor_id:  self.actor_id.as_str(),
                target_id: self.target_id.as_str(),
            }
        })?;
        self.actor_follows_target_since = None;
        self.pending_events.push(DomainEvent::ProfileUnfollowed(ProfileUnfollowed {
            actor_id:      self.actor_id,
            target_id:     self.target_id,
            unfollowed_at: Utc::now(),
        }));
        Ok(followed_at)
    }

    /// Records that the actor blocks the target, severs any existing follows
    /// in both directions, and returns the timestamps of severed edges.
    ///
    /// # Errors
    ///
    /// - [`SocialGraphError::AlreadyBlocked`] if the block already exists.
    pub fn block(&mut self) -> Result<SeveredFollows, SocialGraphError> {
        if self.actor_blocks_target {
            return Err(SocialGraphError::AlreadyBlocked {
                actor_id:  self.actor_id.as_str(),
                target_id: self.target_id.as_str(),
            });
        }
        let severed = SeveredFollows {
            actor_to_target: self.actor_follows_target_since.take(),
            target_to_actor: self.target_follows_actor_since.take(),
        };
        self.actor_blocks_target = true;
        let now = Utc::now();
        self.pending_events.push(DomainEvent::ProfileBlocked(ProfileBlocked {
            actor_id:              self.actor_id,
            target_id:             self.target_id,
            blocked_at:            now,
            severed_actor_follow:  severed.actor_to_target.is_some(),
            severed_target_follow: severed.target_to_actor.is_some(),
        }));
        Ok(severed)
    }

    /// Removes the block that the actor placed on the target.
    ///
    /// Does not restore severed follows (the user must re-follow explicitly).
    ///
    /// # Errors
    ///
    /// - [`SocialGraphError::NotBlocked`] if no block exists.
    pub fn unblock(&mut self) -> Result<(), SocialGraphError> {
        if !self.actor_blocks_target {
            return Err(SocialGraphError::NotBlocked {
                actor_id:  self.actor_id.as_str(),
                target_id: self.target_id.as_str(),
            });
        }
        self.actor_blocks_target = false;
        self.pending_events.push(DomainEvent::ProfileUnblocked(ProfileUnblocked {
            actor_id:     self.actor_id,
            target_id:    self.target_id,
            unblocked_at: Utc::now(),
        }));
        Ok(())
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    pub fn status(&self) -> RelationStatus {
        if self.actor_blocks_target {
            return RelationStatus::Blocking;
        }
        if self.target_blocks_actor {
            return RelationStatus::BlockedBy;
        }
        match (
            self.actor_follows_target_since.is_some(),
            self.target_follows_actor_since.is_some(),
        ) {
            (true,  true)  => RelationStatus::MutualFollow,
            (true,  false) => RelationStatus::Following,
            (false, true)  => RelationStatus::FollowedBy,
            (false, false) => RelationStatus::None,
        }
    }

    pub fn actor_follows_target_since(&self) -> Option<DateTime<Utc>> {
        self.actor_follows_target_since
    }

    pub fn target_follows_actor_since(&self) -> Option<DateTime<Utc>> {
        self.target_follows_actor_since
    }

    pub fn actor_blocks_target(&self) -> bool {
        self.actor_blocks_target
    }

    pub fn target_blocks_actor(&self) -> bool {
        self.target_blocks_actor
    }

    /// Drains and returns all accumulated domain events.
    pub fn take_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
