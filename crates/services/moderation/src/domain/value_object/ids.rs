//! Identifier value objects for the moderation domain.
//!
//! Two flavours, deliberately chosen per aggregate:
//! * **UUIDv7** (time-ordered, freshly minted) for records created exactly once:
//!   [`DecisionId`], [`EnforcementId`], [`AppealId`].
//! * **UUIDv5** (deterministic, content-addressed) for entities that MUST
//!   deduplicate under at-least-once redelivery: [`CaseId`] (keyed by the
//!   subject) and [`ReportId`] (keyed by reporter + subject). Recomputing the id
//!   from the same inputs yields the same UUID, so a redelivered event upserts
//!   the same row instead of creating a duplicate (the idempotency rule of the
//!   Consumer Runtime Standard).

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::SubjectRef;
use crate::error::ModerationError;

/// Fixed namespace for all of moderation's deterministic (v5) identifiers.
/// The bytes spell `MODERATIONNSUUID`; it never changes (changing it would
/// re-key every deterministic id and break dedup).
pub const MODERATION_NAMESPACE: Uuid = Uuid::from_bytes(*b"MODERATIONNSUUID");

/// The responsible account behind a subject (a content author or the account
/// itself). Backed by a UUID to match the rest of the fleet's account ids.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActorId(Uuid);

impl ActorId {
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl fmt::Debug for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorId({})", self.0.hyphenated())
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for ActorId {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ModerationError::InvalidIdentifier(s.to_owned()))
    }
}

/// Macro: a freshly-minted UUIDv7 identifier newtype (record created once).
macro_rules! uuid_v7_id {
    ($name:ident, $label:literal) => {
        #[doc = concat!("Opaque ", $label, " identifier (UUIDv7, time-ordered).")]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a fresh UUIDv7 identifier.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID from a trusted source (storage row).
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            pub fn as_str(&self) -> String {
                self.0.hyphenated().to_string()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0.hyphenated())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.hyphenated())
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ModerationError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|_| ModerationError::InvalidIdentifier(s.to_owned()))
            }
        }
    };
}

uuid_v7_id!(DecisionId, "decision");
uuid_v7_id!(EnforcementId, "enforcement action");
uuid_v7_id!(AppealId, "appeal");

/// Review-case identifier. **Deterministic** (UUIDv5 of the subject) so opening a
/// case for the same subject twice — e.g. a redelivered content event — yields
/// the same id and upserts rather than duplicating.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaseId(Uuid);

impl CaseId {
    /// Derives the deterministic case id for a subject.
    pub fn for_subject(subject: &SubjectRef) -> Self {
        Self(Uuid::new_v5(
            &MODERATION_NAMESPACE,
            subject.canonical_key().as_bytes(),
        ))
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl fmt::Debug for CaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CaseId({})", self.0.hyphenated())
    }
}

impl fmt::Display for CaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

/// User-report identifier. **Deterministic** (UUIDv5 of reporter + subject) so a
/// reporter filing the same report twice collapses to one record.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReportId(Uuid);

impl ReportId {
    /// Derives the deterministic report id for a (reporter, subject) pair.
    pub fn for_report(reporter: ActorId, subject: &SubjectRef) -> Self {
        let name = format!("{}|{}", reporter, subject.canonical_key());
        Self(Uuid::new_v5(&MODERATION_NAMESPACE, name.as_bytes()))
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl fmt::Debug for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReportId({})", self.0.hyphenated())
    }
}

impl fmt::Display for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_object::EntityType;

    fn subject() -> SubjectRef {
        SubjectRef::new(
            EntityType::Post,
            "post-123",
            ActorId::from_uuid(Uuid::nil()),
            "feed",
        )
        .unwrap()
    }

    #[test]
    fn v7_ids_are_unique_and_versioned() {
        let a = DecisionId::new();
        let b = DecisionId::new();
        assert_ne!(a, b);
        assert_eq!(a.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn case_id_is_deterministic_per_subject() {
        let s = subject();
        assert_eq!(CaseId::for_subject(&s), CaseId::for_subject(&s));
        assert_eq!(CaseId::for_subject(&s).as_uuid().get_version_num(), 5);
    }

    #[test]
    fn case_id_differs_by_subject() {
        let s1 = subject();
        let s2 = SubjectRef::new(
            EntityType::Post,
            "post-999",
            ActorId::from_uuid(Uuid::nil()),
            "feed",
        )
        .unwrap();
        assert_ne!(CaseId::for_subject(&s1), CaseId::for_subject(&s2));
    }

    #[test]
    fn report_id_dedups_same_reporter_and_subject() {
        let r = ActorId::from_uuid(Uuid::from_u128(7));
        let s = subject();
        assert_eq!(ReportId::for_report(r, &s), ReportId::for_report(r, &s));
        // different reporter ⇒ different id
        let r2 = ActorId::from_uuid(Uuid::from_u128(8));
        assert_ne!(ReportId::for_report(r, &s), ReportId::for_report(r2, &s));
    }

    #[test]
    fn id_parse_rejects_garbage() {
        assert!(matches!(
            DecisionId::try_from("nope").unwrap_err(),
            ModerationError::InvalidIdentifier(_)
        ));
    }
}
