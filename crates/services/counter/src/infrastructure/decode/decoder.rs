//! Pure wire → [`Observation`] distillation. One inbound event yields zero or more
//! observations (a view becomes both a `View` sum and a `UniqueViewer` member; a
//! follow becomes a `Follower` on the followee and a `Following` on the follower).
//!
//! This is engine-free and fully unit-tested; the consumer wiring (Phase 5) owns
//! deserialization (so poison bytes dead-letter before reaching here) and then
//! calls these `map_*` functions on the already-decoded wire enum.

use chrono::{DateTime, TimeZone, Utc};

use crate::domain::{EntityId, EntityKind, EntityRef, MemberId, Metric, Observation};
use crate::error::CounterError;
use crate::infrastructure::decode::wire::{FollowWire, HitWire, ReactionWire};

fn at(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

fn entity(kind: &str, id: &str) -> Result<EntityRef, CounterError> {
    Ok(EntityRef::new(
        EntityKind::try_from_str(kind)?,
        EntityId::new(id)?,
    ))
}

/// `view.v1.events` → a `View` sum (`+1`) plus, when the viewer is known, a
/// `UniqueViewer` member for the HyperLogLog.
pub fn map_view(wire: HitWire) -> Result<Vec<Observation>, CounterError> {
    let e = entity(&wire.entity_type, &wire.entity_id)?;
    let mut out = vec![Observation::sum(e.clone(), Metric::View, 1, at(wire.occurred_at_ms))?];
    if let Some(actor) = wire.actor_id {
        out.push(Observation::unique(
            e,
            Metric::UniqueViewer,
            MemberId::new(actor)?,
            at(wire.occurred_at_ms),
        )?);
    }
    Ok(out)
}

/// `impression.v1.events` → an `Impression` sum plus, when the actor is known, a
/// `Reach` member (unique accounts reached).
pub fn map_impression(wire: HitWire) -> Result<Vec<Observation>, CounterError> {
    let e = entity(&wire.entity_type, &wire.entity_id)?;
    let mut out = vec![Observation::sum(
        e.clone(),
        Metric::Impression,
        1,
        at(wire.occurred_at_ms),
    )?];
    if let Some(actor) = wire.actor_id {
        out.push(Observation::unique(
            e,
            Metric::Reach,
            MemberId::new(actor)?,
            at(wire.occurred_at_ms),
        )?);
    }
    Ok(out)
}

/// `click.v1.events` → a `Click` sum (`+1`). Clicks are not deduplicated.
pub fn map_click(wire: HitWire) -> Result<Vec<Observation>, CounterError> {
    let e = entity(&wire.entity_type, &wire.entity_id)?;
    Ok(vec![Observation::sum(
        e,
        Metric::Click,
        1,
        at(wire.occurred_at_ms),
    )?])
}

/// `engagement.reactions` → a `Like` magnitude on the post. A brand-new reaction
/// is `+1`, a removal is `-1`, and a *replacement* (an upsert carrying a prior
/// `old_kind`) is a no-op — the reaction count did not change.
pub fn map_reaction(wire: ReactionWire) -> Result<Vec<Observation>, CounterError> {
    match wire {
        ReactionWire::Upserted(e) if e.old_kind.is_some() => Ok(Vec::new()), // replacement
        ReactionWire::Upserted(e) => Ok(vec![Observation::sum(
            entity("post", &e.post_id)?,
            Metric::Like,
            1,
            at(e.event_at_ms),
        )?]),
        ReactionWire::Removed(e) => Ok(vec![Observation::sum(
            entity("post", &e.post_id)?,
            Metric::Like,
            -1,
            at(e.event_at_ms),
        )?]),
    }
}

/// A social-graph follow change → a `Follower` magnitude on the followee and a
/// `Following` magnitude on the follower (`+1` follow, `-1` unfollow), so both
/// counts stay consistent from a single event.
pub fn map_follow(wire: FollowWire) -> Result<Vec<Observation>, CounterError> {
    let (change, amount) = match wire {
        FollowWire::Followed(c) => (c, 1),
        FollowWire::Unfollowed(c) => (c, -1),
    };
    let when = at(change.occurred_at_ms);
    Ok(vec![
        Observation::sum(
            entity("profile", &change.followee_id)?,
            Metric::Follower,
            amount,
            when,
        )?,
        Observation::sum(
            entity("profile", &change.follower_id)?,
            Metric::Following,
            amount,
            when,
        )?,
    ])
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::infrastructure::decode::wire::{
        FollowChangeWire, ReactionRemovedWire, ReactionUpsertedWire,
    };

    #[test]
    fn view_with_viewer_yields_view_and_unique() {
        let obs = map_view(HitWire {
            entity_type: "post".into(),
            entity_id: "p1".into(),
            actor_id: Some("viewer-1".into()),
            occurred_at_ms: 1_000,
        })
        .unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].metric, Metric::View);
        assert_eq!(obs[0].amount, 1);
        assert_eq!(obs[1].metric, Metric::UniqueViewer);
        assert_eq!(obs[1].unique_member.as_ref().unwrap().as_str(), "viewer-1");
    }

    #[test]
    fn view_without_viewer_is_sum_only() {
        let obs = map_view(HitWire {
            entity_type: "post".into(),
            entity_id: "p1".into(),
            actor_id: None,
            occurred_at_ms: 1_000,
        })
        .unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].metric, Metric::View);
    }

    #[test]
    fn unknown_entity_kind_is_rejected() {
        let err = map_view(HitWire {
            entity_type: "account".into(),
            entity_id: "a1".into(),
            actor_id: None,
            occurred_at_ms: 1,
        })
        .unwrap_err();
        assert_eq!(err.error_code(), "CTR-9001");
    }

    #[test]
    fn new_reaction_is_plus_one_like() {
        let obs = map_reaction(ReactionWire::Upserted(ReactionUpsertedWire {
            post_id: "p1".into(),
            old_kind: None,
            event_at_ms: 1,
        }))
        .unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].metric, Metric::Like);
        assert_eq!(obs[0].amount, 1);
    }

    #[test]
    fn replaced_reaction_is_a_no_op() {
        let obs = map_reaction(ReactionWire::Upserted(ReactionUpsertedWire {
            post_id: "p1".into(),
            old_kind: Some("heart".into()),
            event_at_ms: 1,
        }))
        .unwrap();
        assert!(obs.is_empty());
    }

    #[test]
    fn removed_reaction_is_minus_one_like() {
        let obs = map_reaction(ReactionWire::Removed(ReactionRemovedWire {
            post_id: "p1".into(),
            event_at_ms: 1,
        }))
        .unwrap();
        assert_eq!(obs[0].amount, -1);
    }

    #[test]
    fn follow_updates_both_sides() {
        let obs = map_follow(FollowWire::Followed(FollowChangeWire {
            follower_id: "alice".into(),
            followee_id: "bob".into(),
            occurred_at_ms: 1,
        }))
        .unwrap();
        assert_eq!(obs.len(), 2);
        // followee gains a Follower
        assert_eq!(obs[0].metric, Metric::Follower);
        assert_eq!(obs[0].entity.id.as_str(), "bob");
        assert_eq!(obs[0].amount, 1);
        // follower gains a Following
        assert_eq!(obs[1].metric, Metric::Following);
        assert_eq!(obs[1].entity.id.as_str(), "alice");
    }

    #[test]
    fn unfollow_decrements_both_sides() {
        let obs = map_follow(FollowWire::Unfollowed(FollowChangeWire {
            follower_id: "alice".into(),
            followee_id: "bob".into(),
            occurred_at_ms: 1,
        }))
        .unwrap();
        assert_eq!(obs[0].amount, -1);
        assert_eq!(obs[1].amount, -1);
    }
}
