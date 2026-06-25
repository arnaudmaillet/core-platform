use serde::{Deserialize, Serialize};

use crate::domain::value_object::{ActorId, EntityType};
use crate::error::ModerationError;

/// The thing being moderated — the linchpin reference type. Moderation holds
/// *this*, never the content bytes. `actor_id` is the responsible account;
/// `surface` is the placement (e.g. `"feed"`, `"dm"`, `"profile"`) for
/// surface-scoped policy.
///
/// The [`SubjectRef::canonical_key`] is the stable string used to derive the
/// deterministic [`CaseId`](crate::domain::value_object::CaseId), so its format is
/// part of the dedup contract and must not change once data exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectRef {
    entity_type: EntityType,
    entity_id: String,
    actor_id: ActorId,
    surface: String,
}

impl SubjectRef {
    /// Constructs a subject, rejecting a blank `entity_id`. A blank `surface` is
    /// allowed (some subjects, e.g. account-level, have no placement).
    pub fn new(
        entity_type: EntityType,
        entity_id: impl Into<String>,
        actor_id: ActorId,
        surface: impl Into<String>,
    ) -> Result<Self, ModerationError> {
        let entity_id = entity_id.into();
        if entity_id.trim().is_empty() {
            return Err(ModerationError::InvalidSubjectRef(
                "entity_id must not be empty".into(),
            ));
        }
        Ok(Self {
            entity_type,
            entity_id,
            actor_id,
            surface: surface.into(),
        })
    }

    pub fn entity_type(&self) -> EntityType {
        self.entity_type
    }

    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }

    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    /// Stable string identity of the *content* this subject points at, used to
    /// derive the deterministic case id. Intentionally excludes `actor_id` (it is
    /// derivable from the content and a case is about the entity, not the actor)
    /// and `surface` is included since the same entity on different surfaces is a
    /// distinct moderation subject.
    pub fn canonical_key(&self) -> String {
        format!("{}|{}|{}", self.entity_type.as_str(), self.entity_id, self.surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn actor() -> ActorId {
        ActorId::from_uuid(Uuid::from_u128(1))
    }

    #[test]
    fn rejects_blank_entity_id() {
        assert!(matches!(
            SubjectRef::new(EntityType::Post, "  ", actor(), "feed").unwrap_err(),
            ModerationError::InvalidSubjectRef(_)
        ));
    }

    #[test]
    fn canonical_key_is_stable_and_surface_sensitive() {
        let feed = SubjectRef::new(EntityType::Post, "p1", actor(), "feed").unwrap();
        let dm = SubjectRef::new(EntityType::Post, "p1", actor(), "dm").unwrap();
        assert_eq!(feed.canonical_key(), "post|p1|feed");
        assert_ne!(feed.canonical_key(), dm.canonical_key());
    }

    #[test]
    fn canonical_key_ignores_actor() {
        let a = SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(1)), "feed").unwrap();
        let b = SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(2)), "feed").unwrap();
        assert_eq!(a.canonical_key(), b.canonical_key());
    }
}
