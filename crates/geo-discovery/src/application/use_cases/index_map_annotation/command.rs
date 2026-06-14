// crates/geo_discovery/src/application/commands/index_active_post.rs

use crate::types::PopularityScore;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, ProfileId, Region};
use uuid::Uuid;
#[derive(Debug, Deserialize, Clone)]
pub struct IndexMapAnnotationCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
    pub post_id: PostId,
    pub location: GeoPoint,
    pub post_type: String,
    pub thumbnail_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub popularity_score: PopularityScore,
}

impl IdentifiableCommand for IndexMapAnnotationCommand {
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

impl IndexMapAnnotationCommand {
    pub fn new(
        command_id: Uuid,
        author_id: ProfileId,
        region: Region,
        post_id: PostId,
        location: GeoPoint,
        post_type: String,
        thumbnail_url: Option<String>,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        popularity_score: PopularityScore,
    ) -> Self {
        Self {
            command_id,
            target: CommandTarget::stateless(author_id),
            region,
            post_id,
            location,
            post_type,
            thumbnail_url,
            created_at,
            expires_at,
            popularity_score,
        }
    }
}
