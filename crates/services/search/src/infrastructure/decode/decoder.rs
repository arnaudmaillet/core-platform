//! Pure wire→domain decoding. No I/O, fully unit-testable — this is where the
//! Phase-2 design's "decode layer distills source events" promise is kept:
//! moderation actions are reduced to visibility transitions, thin content events
//! are marked for hydration, and everything search ignores collapses to `Ignore`.

use super::wire::{ModerationWireEvent, PostWireEvent};
use crate::domain::{
    EntityDeletion, EntityKind, ModerationEvent, PostEvent, SourceEvent, VisibilityChange,
};
use crate::error::SearchError;

/// The outcome of decoding one wire message.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// Directly projectable — no content fetch needed (a delete, a visibility flip).
    Ready(SourceEvent),
    /// A content-bearing notification whose body must be hydrated from the source
    /// service (the wire event is thin) before it can be projected.
    NeedsContent(ContentRef),
    /// An event this consumer does not act on — committed as a benign no-op.
    Ignore,
}

/// What the hydrator (Phase 5) needs to fetch the authoritative snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentRef {
    pub kind: EntityKind,
    pub id: String,
    pub author_id: String,
    /// The source revision (event time, ms) → the document's content version.
    pub revision: u64,
}

/// Decode a `post.v1.events` message body. Used by the unit tests; the live
/// consumer lets `run_consumer` deserialize and calls [`map_post`] directly.
pub fn decode_post(json: &[u8]) -> Result<Decoded, SearchError> {
    let event: PostWireEvent = serde_json::from_slice(json).map_err(|e| decode_err("post", e))?;
    Ok(map_post(event))
}

/// Map an already-deserialized post wire event to a [`Decoded`] (pure, infallible).
pub fn map_post(event: PostWireEvent) -> Decoded {
    match event {
        PostWireEvent::PostPublished(e) => Decoded::NeedsContent(ContentRef {
            kind: EntityKind::Post,
            id: e.post_id,
            author_id: e.profile_id,
            revision: ms_to_revision(e.published_at_ms),
        }),
        PostWireEvent::PostUpdated(e) => Decoded::NeedsContent(ContentRef {
            kind: EntityKind::Post,
            id: e.post_id,
            author_id: e.profile_id,
            revision: ms_to_revision(e.updated_at_ms),
        }),
        PostWireEvent::PostDeleted(e) => {
            Decoded::Ready(SourceEvent::Post(PostEvent::Deleted(EntityDeletion {
                id: e.post_id,
            })))
        }
    }
}

/// Decode a `moderation.v1.events` message body. Used by the unit tests; the live
/// consumer lets `run_consumer` deserialize and calls [`map_moderation`] directly.
pub fn decode_moderation(json: &[u8]) -> Result<Decoded, SearchError> {
    let event: ModerationWireEvent =
        serde_json::from_slice(json).map_err(|e| decode_err("moderation", e))?;
    Ok(map_moderation(event))
}

/// Map an already-deserialized moderation wire event to a [`Decoded`], distilling
/// it to a visibility transition (or ignoring it). Pure, infallible.
pub fn map_moderation(event: ModerationWireEvent) -> Decoded {
    match event {
        ModerationWireEvent::EnforcementApplied(e) => {
            // Only content-visibility actions hide search results; actor-level
            // penalties (Warn/Restrict/Suspend/Ban) are not search's concern.
            if is_content_action(e.action.as_deref()) {
                Decoded::Ready(SourceEvent::Moderation(ModerationEvent::VisibilityRevoked(
                    VisibilityChange {
                        kind: map_entity(&e.subject.entity_type),
                        id: e.subject.entity_id,
                        occurred_at: e.occurred_at,
                    },
                )))
            } else {
                Decoded::Ignore
            }
        }
        // A reversal carries no action; restoring visibility on a mappable entity is
        // harmless if it was never hidden, and correct if it was.
        ModerationWireEvent::EnforcementReversed(e) => match map_entity(&e.subject.entity_type) {
            Some(kind) => Decoded::Ready(SourceEvent::Moderation(
                ModerationEvent::VisibilityRestored(VisibilityChange {
                    kind: Some(kind),
                    id: e.subject.entity_id,
                    occurred_at: e.occurred_at,
                }),
            )),
            None => Decoded::Ignore,
        },
        ModerationWireEvent::Other => Decoded::Ignore,
    }
}

/// Map moderation's `EntityType` variant name to a search index kind. Entities that
/// have no search index (Comment, ChatMessage, Media, Account) map to `None` — the
/// projector then skips the visibility change.
fn map_entity(entity_type: &str) -> Option<EntityKind> {
    match entity_type {
        "Post" => Some(EntityKind::Post),
        "Profile" => Some(EntityKind::Profile),
        _ => None,
    }
}

fn is_content_action(action: Option<&str>) -> bool {
    matches!(action, Some("RemoveContent") | Some("VisibilityLimit"))
}

fn ms_to_revision(ms: i64) -> u64 {
    if ms < 0 { 0 } else { ms as u64 }
}

fn decode_err(topic: &str, err: serde_json::Error) -> SearchError {
    SearchError::EventDecodeFailed {
        topic: format!("{topic}.v1.events"),
        reason: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn post_published_needs_hydration() {
        let json = br#"{"type":"PostPublished","post_id":"post-1","profile_id":"acct-9","kind":"text","published_at_ms":1700000000000}"#;
        let decoded = decode_post(json).unwrap();
        assert_eq!(
            decoded,
            Decoded::NeedsContent(ContentRef {
                kind: EntityKind::Post,
                id: "post-1".to_owned(),
                author_id: "acct-9".to_owned(),
                revision: 1_700_000_000_000,
            })
        );
    }

    #[test]
    fn post_deleted_is_directly_projectable() {
        let json = br#"{"type":"PostDeleted","post_id":"post-1","profile_id":"acct-9","deleted_at_ms":1700000000000}"#;
        let decoded = decode_post(json).unwrap();
        assert_eq!(
            decoded,
            Decoded::Ready(SourceEvent::Post(PostEvent::Deleted(EntityDeletion {
                id: "post-1".to_owned()
            })))
        );
    }

    #[test]
    fn malformed_post_event_is_a_decode_error() {
        let err = decode_post(br#"{"type":"Nonsense"}"#).unwrap_err();
        assert_eq!(err.error_code(), "SCH-8001");
        assert!(!err.is_retryable(), "a poison message must DLQ, not retry");
    }

    #[test]
    fn remove_content_on_a_post_revokes_visibility() {
        let json = br#"{"type":"enforcement_applied","subject":{"entity_type":"Post","entity_id":"post-1","actor_id":"acct-9","surface":"feed"},"actor_id":"acct-9","action":"RemoveContent","version":3,"applied_at":"2026-06-26T12:00:00Z","occurred_at":"2026-06-26T12:00:00Z","correlation_id":"00000000-0000-0000-0000-000000000000"}"#;
        let decoded = decode_moderation(json).unwrap();
        match decoded {
            Decoded::Ready(SourceEvent::Moderation(ModerationEvent::VisibilityRevoked(v))) => {
                assert_eq!(v.kind, Some(EntityKind::Post));
                assert_eq!(v.id, "post-1");
            }
            other => panic!("expected a visibility revoke, got {other:?}"),
        }
    }

    #[test]
    fn actor_level_action_is_ignored() {
        let json = br#"{"type":"enforcement_applied","subject":{"entity_type":"Account","entity_id":"acct-9","actor_id":"acct-9","surface":""},"actor_id":"acct-9","action":"Suspend","version":1,"applied_at":"2026-06-26T12:00:00Z","occurred_at":"2026-06-26T12:00:00Z","correlation_id":"00000000-0000-0000-0000-000000000000"}"#;
        assert_eq!(decode_moderation(json).unwrap(), Decoded::Ignore);
    }

    #[test]
    fn content_action_on_unmapped_entity_skips_via_none_kind() {
        let json = br#"{"type":"enforcement_applied","subject":{"entity_type":"Comment","entity_id":"c-1","actor_id":"acct-9","surface":""},"actor_id":"acct-9","action":"RemoveContent","version":1,"applied_at":"2026-06-26T12:00:00Z","occurred_at":"2026-06-26T12:00:00Z","correlation_id":"00000000-0000-0000-0000-000000000000"}"#;
        match decode_moderation(json).unwrap() {
            Decoded::Ready(SourceEvent::Moderation(ModerationEvent::VisibilityRevoked(v))) => {
                assert_eq!(v.kind, None);
            }
            other => panic!("expected a (none-kind) revoke, got {other:?}"),
        }
    }

    #[test]
    fn reversal_restores_visibility() {
        let json = br#"{"type":"enforcement_reversed","subject":{"entity_type":"Profile","entity_id":"prof-1","actor_id":"acct-9","surface":""},"actor_id":"acct-9","version":4,"occurred_at":"2026-06-26T12:00:00Z","correlation_id":"00000000-0000-0000-0000-000000000000"}"#;
        match decode_moderation(json).unwrap() {
            Decoded::Ready(SourceEvent::Moderation(ModerationEvent::VisibilityRestored(v))) => {
                assert_eq!(v.kind, Some(EntityKind::Profile));
            }
            other => panic!("expected a visibility restore, got {other:?}"),
        }
    }

    #[test]
    fn other_moderation_events_are_ignored() {
        let json = br#"{"type":"case_opened","subject":{"entity_type":"Post","entity_id":"post-1","actor_id":"acct-9","surface":"feed"}}"#;
        assert_eq!(decode_moderation(json).unwrap(), Decoded::Ignore);
    }
}
