// crates/social/src/domain/types/follow_relation_id.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::Identifier;
use shared_kernel::types::ProfileId;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FollowRelationId {
    follower_id: ProfileId,
    following_id: ProfileId,
}

impl FollowRelationId {
    pub fn new(follower_id: ProfileId, following_id: ProfileId) -> Self {
        Self {
            follower_id,
            following_id,
        }
    }

    pub fn follower_id(&self) -> ProfileId {
        self.follower_id
    }
    pub fn following_id(&self) -> ProfileId {
        self.following_id
    }
}

impl std::fmt::Display for FollowRelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.follower_id, self.following_id)
    }
}

impl Identifier for FollowRelationId {
    fn as_uuid(&self) -> Uuid {
        let namespace = Uuid::NAMESPACE_OID;
        let name = format!("{}:{}", self.follower_id, self.following_id);

        Uuid::new_v5(&namespace, name.as_bytes())
    }

    fn as_string(&self) -> String {
        self.to_string()
    }

    fn from_uuid(_uuid: Uuid) -> Self {
        panic!("Cannot reconstruct a composite FollowRelationId from a single raw UUID")
    }

    fn identifier_scope() -> &'static str {
        "FollowRelation"
    }
}
