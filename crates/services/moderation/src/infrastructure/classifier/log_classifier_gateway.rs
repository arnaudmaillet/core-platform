use async_trait::async_trait;
use tracing::debug;

use crate::application::port::ClassifierGateway;
use crate::domain::value_object::SubjectRef;
use crate::error::ModerationError;

/// A no-op [`ClassifierGateway`] that traces classification requests instead of
/// dispatching them. Stands in until the internal classifier services exist; the
/// graduated engine runs on deterministic rules in the meantime.
#[derive(Default)]
pub struct LogClassifierGateway;

#[async_trait]
impl ClassifierGateway for LogClassifierGateway {
    async fn request_classification(&self, subject: &SubjectRef) -> Result<(), ModerationError> {
        debug!(
            entity_type = %subject.entity_type(),
            entity_id = subject.entity_id(),
            "classification requested (log stub)"
        );
        Ok(())
    }
}
