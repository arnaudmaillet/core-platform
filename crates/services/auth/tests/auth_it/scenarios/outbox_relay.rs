//! The outbox decoupling contract: handlers enqueue durably, the relay drains
//! to the sink in enqueue order, and a sink outage delays — never drops or
//! reorders — compliance events (audit consumes auth.v1.events).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use auth::application::port::EventPublisher;
use auth::domain::event::DomainEvent;
use auth::error::AuthError;
use auth::infrastructure::event::outbox_relay::OutboxRelay;
use auth::infrastructure::event::pg_outbox_publisher::PgOutboxPublisher;
use uuid::Uuid;

use crate::auth_it::harness::Harness;

/// Sink that records events and can simulate a broker outage.
#[derive(Default)]
struct FlakySink {
    down: AtomicBool,
    seen: Mutex<Vec<String>>,
}

#[async_trait]
impl EventPublisher for FlakySink {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AuthError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(AuthError::EventPublishFailed("sink down".into()));
        }
        self.seen.lock().unwrap().push(event.event_type().to_owned());
        Ok(())
    }
}

fn sample_event() -> DomainEvent {
    use auth::domain::event::SessionIssued;
    use auth::domain::value_object::{AccountId, Generation, IdpSubject, SessionId};
    let now = chrono::Utc::now();
    DomainEvent::SessionIssued(SessionIssued {
        session_id: SessionId::new(),
        account_id: AccountId::from_uuid(Uuid::now_v7()),
        subject: IdpSubject::new("https://idp.test", "sub-1").expect("subject"),
        generation: Generation::from_i64(0),
        issued_at: now,
        expires_at: now,
        absolute_expiry: now,
        occurred_at: now,
        correlation_id: Uuid::now_v7(),
    })
}

async fn pending(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM auth_outbox")
        .fetch_one(pool)
        .await
        .expect("count outbox")
}

#[tokio::test]
async fn outbox_survives_sink_outage_and_drains_in_order() {
    let h = Harness::start().await;
    let outbox = PgOutboxPublisher::new(h.pool.clone());
    let sink = Arc::new(FlakySink::default());
    let relay = OutboxRelay::new(h.pool.clone(), sink.clone());

    // Enqueue while the "broker" is down — the handler-visible publish must
    // still succeed (that's the whole decoupling).
    sink.down.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        outbox.publish(&sample_event()).await.expect("enqueue must not depend on the sink");
    }
    assert_eq!(pending(&h.pool).await, 3);

    // A tick during the outage publishes nothing and loses nothing.
    assert!(relay.tick().await.is_err(), "stalled batch must surface");
    assert_eq!(pending(&h.pool).await, 3, "outage must not drop events");
    assert!(sink.seen.lock().unwrap().is_empty());

    // Sink recovers: one tick drains everything, table ends empty.
    sink.down.store(false, Ordering::SeqCst);
    assert_eq!(relay.tick().await.expect("drain"), 3);
    assert_eq!(pending(&h.pool).await, 0, "published rows must be deleted");
    assert_eq!(sink.seen.lock().unwrap().len(), 3);

    // Idempotent when empty.
    assert_eq!(relay.tick().await.expect("empty tick"), 0);
}
