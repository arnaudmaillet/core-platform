//! In-process capturing fake for the event-publisher dependency.
//!
//! Post's [`EventPublisher`] port is the seam to the Kafka backbone. Rather than
//! boot a broker, the suite injects [`CapturingPublisher`]: it records a label for
//! every emitted [`DomainEvent`] so a scenario can assert that, e.g., publishing a
//! post emitted exactly one `PostPublished`.

use std::sync::Mutex;

use async_trait::async_trait;

use post::application::port::EventPublisher;
use post::domain::event::DomainEvent;
use post::error::PostError;

/// A capturing, never-failing stand-in for the Kafka event publisher.
#[derive(Default)]
pub struct CapturingPublisher {
    labels: Mutex<Vec<String>>,
}

impl CapturingPublisher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of the event labels emitted so far, in order.
    pub fn labels(&self) -> Vec<String> {
        self.labels.lock().unwrap().clone()
    }

    /// How many times `label` has been emitted.
    pub fn count(&self, label: &str) -> usize {
        self.labels.lock().unwrap().iter().filter(|l| l.as_str() == label).count()
    }
}

#[async_trait]
impl EventPublisher for CapturingPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), PostError> {
        let label = match event {
            DomainEvent::PostPublished(_) => "published",
            DomainEvent::PostUpdated(_) => "updated",
            DomainEvent::PostDeleted(_) => "deleted",
        };
        self.labels.lock().unwrap().push(label.to_owned());
        Ok(())
    }
}
