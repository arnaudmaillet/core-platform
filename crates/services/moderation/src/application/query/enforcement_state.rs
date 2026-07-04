use std::sync::Arc;

use chrono::{DateTime, Utc};
use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::{EnforcementProjection, EnforcementRepository};
use crate::domain::aggregate::EnforcementAction;
use crate::domain::value_object::ActorId;
use crate::error::ModerationError;

/// INTERNAL, discouraged on the hot path — the fleet reads enforcement via Plane B
/// (events + Redis projection), not this RPC. Exists for back-office/cold reads.
#[derive(Debug, Clone)]
pub struct GetEnforcementStateQuery {
    pub actor_id: ActorId,
}

impl Query for GetEnforcementStateQuery {
    type Response = EnforcementStateView;
}

/// Coarse "is this actor restricted" plus the active enforcement records.
#[derive(Debug, Clone)]
pub struct EnforcementStateView {
    pub actor_restricted: bool,
    pub active_enforcements: Vec<EnforcementAction>,
}

pub struct GetEnforcementStateHandler {
    projection: Arc<dyn EnforcementProjection>,
    enforcements: Arc<dyn EnforcementRepository>,
}

impl GetEnforcementStateHandler {
    pub fn new(
        projection: Arc<dyn EnforcementProjection>,
        enforcements: Arc<dyn EnforcementRepository>,
    ) -> Self {
        Self { projection, enforcements }
    }

    /// Clock-injected core, for deterministic tests.
    pub async fn handle_at(
        &self,
        envelope: Envelope<GetEnforcementStateQuery>,
        now: DateTime<Utc>,
    ) -> Result<EnforcementStateView, ModerationError> {
        let actor_id = envelope.payload.actor_id;
        let actor_restricted = self.projection.is_actor_restricted(&actor_id).await?;
        let active_enforcements = self
            .enforcements
            .list_active_for_actor(&actor_id)
            .await?
            .into_iter()
            .filter(|e| e.is_active(now))
            .collect();
        Ok(EnforcementStateView { actor_restricted, active_enforcements })
    }
}

impl QueryHandler<GetEnforcementStateQuery> for GetEnforcementStateHandler {
    type Error = ModerationError;

    async fn handle(
        &self,
        envelope: Envelope<GetEnforcementStateQuery>,
    ) -> Result<EnforcementStateView, Self::Error> {
        self.handle_at(envelope, Utc::now()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::command::{DecideCaseCommand, OpenCaseCommand};
    use crate::application::fakes::{t0, Fixture};
    use crate::domain::value_object::{ActionType, EntityType, PolicyCategory, SubjectRef};
    use uuid::Uuid;

    fn subject() -> SubjectRef {
        SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(1)), "feed").unwrap()
    }

    #[tokio::test]
    async fn reports_active_actor_restriction() {
        let fx = Fixture::new();
        let open = Envelope::new(
            Uuid::now_v7(),
            OpenCaseCommand { subject: subject(), category: PolicyCategory::Hate, queue: "q".into(), priority: "p".into() },
        );
        let case = fx.open_case_handler().handle(open, t0()).await.unwrap().case;
        let decide = Envelope::new(
            Uuid::now_v7(),
            DecideCaseCommand {
                case_id: case.id(),
                action: ActionType::Suspend,
                category: PolicyCategory::Hate,
                rationale: "x".into(),
                reviewer_id: "r".into(),
                policy_version: "2026.06.1".into(),
            },
        );
        fx.decide_handler().handle(decide, t0()).await.unwrap();

        let q = Envelope::new(Uuid::now_v7(), GetEnforcementStateQuery { actor_id: subject().actor_id() });
        let view = fx.enforcement_state_handler().handle_at(q, t0()).await.unwrap();
        assert!(view.actor_restricted);
        assert_eq!(view.active_enforcements.len(), 1);
    }

    #[tokio::test]
    async fn clean_actor_is_unrestricted() {
        let fx = Fixture::new();
        let q = Envelope::new(Uuid::now_v7(), GetEnforcementStateQuery { actor_id: subject().actor_id() });
        let view = fx.enforcement_state_handler().handle_at(q, t0()).await.unwrap();
        assert!(!view.actor_restricted);
        assert!(view.active_enforcements.is_empty());
    }
}
