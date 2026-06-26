use std::sync::Arc;

use async_trait::async_trait;
use scylla::statement::unprepared::Statement;
use scylla::value::CqlTimestamp;
use scylla_storage::{ScyllaClient, ScyllaStorageError};
use uuid::Uuid;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::domain::value_object::SubjectRef;
use crate::error::ModerationError;

const INSERT_CQL: &str = r#"
    INSERT INTO moderation.evidence_history
        (actor_id, occurred_at, event_id, event_type, entity_type, entity_id, category, action, event)
    VALUES (?,?,?,?,?,?,?,?,?)
"#;

/// ScyllaDB append-only **evidence history** — the denormalized, per-actor audit
/// feed projected from the moderation event stream (Postgres remains the system of
/// record). It implements [`EventPublisher`] so it composes as a fan-out sink
/// alongside the Kafka publisher: every event moderation emits is also retained
/// here for transparency reporting, law-enforcement preservation, and training.
#[derive(Clone)]
pub struct ScyllaEvidenceHistory {
    client: Arc<ScyllaClient>,
}

impl ScyllaEvidenceHistory {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }
}

/// The flat projection of an event for the typed history columns.
struct HistoryRow {
    occurred_at: i64,
    entity_type: String,
    entity_id: String,
    category: String,
    action: String,
}

fn subject_cols(subject: &SubjectRef) -> (String, String) {
    (subject.entity_type().as_str().to_owned(), subject.entity_id().to_owned())
}

fn project(event: &DomainEvent) -> HistoryRow {
    match event {
        DomainEvent::CaseOpened(e) => {
            let (et, eid) = subject_cols(&e.subject);
            HistoryRow {
                occurred_at: e.occurred_at.timestamp_millis(),
                entity_type: et,
                entity_id: eid,
                category: e.category.as_str().to_owned(),
                action: String::new(),
            }
        }
        DomainEvent::CaseResolved(e) => HistoryRow {
            occurred_at: e.occurred_at.timestamp_millis(),
            entity_type: String::new(),
            entity_id: String::new(),
            category: e.category.as_str().to_owned(),
            action: e.action.as_str().to_owned(),
        },
        DomainEvent::DecisionRecorded(e) => {
            let (et, eid) = subject_cols(&e.subject);
            HistoryRow {
                occurred_at: e.occurred_at.timestamp_millis(),
                entity_type: et,
                entity_id: eid,
                category: e.category.as_str().to_owned(),
                action: e.action.as_str().to_owned(),
            }
        }
        DomainEvent::EnforcementApplied(e) => {
            let (et, eid) = subject_cols(&e.subject);
            HistoryRow {
                occurred_at: e.occurred_at.timestamp_millis(),
                entity_type: et,
                entity_id: eid,
                category: String::new(),
                action: e.action.as_str().to_owned(),
            }
        }
        DomainEvent::EnforcementReversed(e) => {
            let (et, eid) = subject_cols(&e.subject);
            HistoryRow {
                occurred_at: e.occurred_at.timestamp_millis(),
                entity_type: et,
                entity_id: eid,
                category: String::new(),
                action: String::new(),
            }
        }
        DomainEvent::AppealResolved(e) => HistoryRow {
            occurred_at: e.occurred_at.timestamp_millis(),
            entity_type: String::new(),
            entity_id: String::new(),
            category: String::new(),
            action: String::new(),
        },
    }
}

#[async_trait]
impl EventPublisher for ScyllaEvidenceHistory {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ModerationError> {
        let row = project(event);
        let payload = serde_json::to_string(event).map_err(|e| ModerationError::EventPublishFailed(e.to_string()))?;

        self.client
            .session
            .execute_unpaged(
                Statement::new(INSERT_CQL),
                (
                    event.actor_id().as_uuid(),
                    CqlTimestamp(row.occurred_at),
                    Uuid::now_v7(),
                    event.event_type(),
                    row.entity_type,
                    row.entity_id,
                    row.category,
                    row.action,
                    payload,
                ),
            )
            .await
            .map_err(|e| ModerationError::History(ScyllaStorageError::from(e)))?;
        Ok(())
    }
}
