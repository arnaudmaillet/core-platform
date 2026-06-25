use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::CaseRepository;
use crate::domain::aggregate::Case;
use crate::domain::value_object::CaseStatus;
use crate::error::ModerationError;

/// List the review queue (paged) for triage UIs.
#[derive(Debug, Clone)]
pub struct ListQueueQuery {
    pub queue: String,
    pub status_filter: Option<CaseStatus>,
    pub limit: usize,
}

impl Query for ListQueueQuery {
    type Response = Vec<Case>;
}

pub struct ListQueueHandler {
    cases: Arc<dyn CaseRepository>,
}

impl ListQueueHandler {
    pub fn new(cases: Arc<dyn CaseRepository>) -> Self {
        Self { cases }
    }
}

impl QueryHandler<ListQueueQuery> for ListQueueHandler {
    type Error = ModerationError;

    async fn handle(&self, envelope: Envelope<ListQueueQuery>) -> Result<Vec<Case>, Self::Error> {
        let q = envelope.payload;
        self.cases.list_queue(&q.queue, q.status_filter, q.limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::command::OpenCaseCommand;
    use crate::application::fakes::{t0, Fixture};
    use crate::domain::value_object::{ActorId, EntityType, PolicyCategory, SubjectRef};
    use uuid::Uuid;

    #[tokio::test]
    async fn lists_open_cases_in_the_queue() {
        let fx = Fixture::new();
        for n in 0..3u128 {
            let subject = SubjectRef::new(EntityType::Post, format!("p{n}"), ActorId::from_uuid(Uuid::from_u128(n + 1)), "feed").unwrap();
            let env = Envelope::new(
                Uuid::now_v7(),
                OpenCaseCommand { subject, category: PolicyCategory::Spam, queue: "default".into(), priority: "normal".into() },
            );
            fx.open_case_handler().handle(env, t0()).await.unwrap();
        }
        let q = Envelope::new(
            Uuid::now_v7(),
            ListQueueQuery { queue: "default".into(), status_filter: Some(CaseStatus::Open), limit: 10 },
        );
        let cases = fx.list_queue_handler().handle(q).await.unwrap();
        assert_eq!(cases.len(), 3);
    }
}
