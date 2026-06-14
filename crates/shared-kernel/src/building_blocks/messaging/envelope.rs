// crates/shared-kernel/src/domain/events/envelope.rs

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::borrow::Cow;
use uuid::Uuid;

use crate::{messaging::Event, types::Region};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    pub region: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
    pub metadata: Option<Value>,
}

impl EventEnvelope {
    pub fn wrap(event: &dyn Event, region: Region) -> Self {
        Self {
            id: event.event_id(),
            region: region.to_string(),
            aggregate_type: event.aggregate_type().into_owned(),
            aggregate_id: event.aggregate_id(),
            event_type: event.event_name().into_owned(),
            payload: event.payload(),
            occurred_at: event.occurred_at(),
            metadata: event
                .correlation_id()
                .map(|id| serde_json::json!({ "correlation_id": id })),
        }
    }
}

impl Event for EventEnvelope {
    fn event_id(&self) -> Uuid {
        self.id
    }
    fn event_name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.event_type)
    }
    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.aggregate_type)
    }
    fn aggregate_id(&self) -> String {
        self.aggregate_id.clone()
    }
    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }
    fn payload(&self) -> Value {
        self.payload.clone()
    }
    fn correlation_id(&self) -> Option<Uuid> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("correlation_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
