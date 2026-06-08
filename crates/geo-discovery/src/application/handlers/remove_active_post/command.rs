// crates/geo_discovery/src/application/commands/remove_post_from_map.rs

use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, ProfileId, Region};
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct RemovePostFromMapCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
    pub post_id: PostId,
    pub location: GeoPoint,
    pub created_at: DateTime<Utc>,
}

impl IdentifiableCommand for RemovePostFromMapCommand {
    type Id = ProfileId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }

    fn resolve_cache_key(&self) -> Option<String> {
        None
    }
}

impl RemovePostFromMapCommand {
    pub fn new(
        command_id: Uuid,
        operator_id: ProfileId,
        region: Region,
        post_id: PostId,
        location: GeoPoint,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            command_id,
            target: CommandTarget::stateless(operator_id),
            region,
            post_id,
            location,
            created_at,
        }
    }
}
