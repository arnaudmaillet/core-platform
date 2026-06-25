use async_trait::async_trait;

use crate::domain::value_object::ActorId;
use crate::error::ModerationError;

/// Read port to the `account` service (gRPC adapter, Phase 4). Moderation
/// **decides** actor-level actions; `account` **executes** the suspension/ban
/// lifecycle by consuming the `EnforcementApplied` event. This port is the small
/// synchronous slice moderation still needs: confirming an actor exists before
/// recording an actor-level enforcement against them, so a typo'd id can't create
/// a dangling restriction.
#[async_trait]
pub trait AccountDirectory: Send + Sync + 'static {
    async fn actor_exists(&self, actor_id: &ActorId) -> Result<bool, ModerationError>;
}
