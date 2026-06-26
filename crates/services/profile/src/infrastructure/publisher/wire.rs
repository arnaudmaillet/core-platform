//! Serde wire schema for `profile.v1.events`.
//!
//! The wire contract is decoupled from the domain value objects: a thin,
//! string-typed enum (ids + timestamps, no display content) so a consumer
//! deserializes a stable shape and a VO refactor never breaks the wire.
//! Internally tagged on `type` (the moderation-service convention), keyed by
//! `profile_id`. Events are intentionally **thin** — a consumer that needs the
//! full profile (e.g. search) hydrates it from `GetProfileById`.

use serde::{Deserialize, Serialize};

use crate::domain::event::DomainEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProfileEventWire {
    ProfileCreated {
        profile_id: String,
        account_id: String,
        handle: String,
        profile_kind: String,
        occurred_at_ms: i64,
    },
    ProfileUpdated {
        profile_id: String,
        occurred_at_ms: i64,
    },
    HandleChanged {
        profile_id: String,
        new_handle: String,
        occurred_at_ms: i64,
    },
    ProfileVerified {
        profile_id: String,
        occurred_at_ms: i64,
    },
    ProfileHidden {
        profile_id: String,
        masking_reason: String,
        occurred_at_ms: i64,
    },
    ProfileRestored {
        profile_id: String,
        occurred_at_ms: i64,
    },
    ProfileDeleted {
        profile_id: String,
        occurred_at_ms: i64,
    },
}

impl ProfileEventWire {
    /// The partition key — guarantees per-profile ordering on the topic.
    pub fn profile_id(&self) -> &str {
        match self {
            ProfileEventWire::ProfileCreated { profile_id, .. }
            | ProfileEventWire::ProfileUpdated { profile_id, .. }
            | ProfileEventWire::HandleChanged { profile_id, .. }
            | ProfileEventWire::ProfileVerified { profile_id, .. }
            | ProfileEventWire::ProfileHidden { profile_id, .. }
            | ProfileEventWire::ProfileRestored { profile_id, .. }
            | ProfileEventWire::ProfileDeleted { profile_id, .. } => profile_id,
        }
    }

    /// The `type` tag, also set as the `event_type` Kafka header for routing.
    pub fn event_type(&self) -> &'static str {
        match self {
            ProfileEventWire::ProfileCreated { .. } => "ProfileCreated",
            ProfileEventWire::ProfileUpdated { .. } => "ProfileUpdated",
            ProfileEventWire::HandleChanged { .. } => "HandleChanged",
            ProfileEventWire::ProfileVerified { .. } => "ProfileVerified",
            ProfileEventWire::ProfileHidden { .. } => "ProfileHidden",
            ProfileEventWire::ProfileRestored { .. } => "ProfileRestored",
            ProfileEventWire::ProfileDeleted { .. } => "ProfileDeleted",
        }
    }
}

impl From<&DomainEvent> for ProfileEventWire {
    fn from(event: &DomainEvent) -> Self {
        match event {
            DomainEvent::ProfileCreated(e) => ProfileEventWire::ProfileCreated {
                profile_id: e.profile_id.to_string(),
                account_id: e.account_id.to_string(),
                handle: e.handle.to_string(),
                profile_kind: e.profile_kind.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::ProfileUpdated(e) => ProfileEventWire::ProfileUpdated {
                profile_id: e.profile_id.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::HandleChanged(e) => ProfileEventWire::HandleChanged {
                profile_id: e.profile_id.to_string(),
                new_handle: e.new_handle.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::ProfileVerified(e) => ProfileEventWire::ProfileVerified {
                profile_id: e.profile_id.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::ProfileHidden(e) => ProfileEventWire::ProfileHidden {
                profile_id: e.profile_id.to_string(),
                masking_reason: e.masking_reason.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::ProfileRestored(e) => ProfileEventWire::ProfileRestored {
                profile_id: e.profile_id.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
            DomainEvent::ProfileDeleted(e) => ProfileEventWire::ProfileDeleted {
                profile_id: e.profile_id.to_string(),
                occurred_at_ms: e.occurred_at.timestamp_millis(),
            },
        }
    }
}
