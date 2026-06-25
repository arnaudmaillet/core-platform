use async_trait::async_trait;

use crate::domain::value_object::SubjectRef;
use crate::error::ModerationError;

/// Outbound gateway to the (future, internal) **classifier services**. Moderation
/// *requests* asynchronous classification and consumes the resulting verdicts back
/// as signals on `moderation.signals` — it never runs ML inference inline. The
/// adapter is fire-and-forget: a request enqueues work, it does not block on a
/// model. Until real classifiers exist this is stubbed, and the graduated engine
/// runs on deterministic rules alone.
#[async_trait]
pub trait ClassifierGateway: Send + Sync + 'static {
    /// Requests asynchronous classification of a subject. The verdict returns later
    /// as a `moderation.signals` event, not as a response here.
    async fn request_classification(&self, subject: &SubjectRef) -> Result<(), ModerationError>;
}
