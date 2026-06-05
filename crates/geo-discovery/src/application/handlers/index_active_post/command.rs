// crates/geo_discovery/src/application/commands/index_active_post.rs

use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, ProfileId, Region};
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct IndexActivePostCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub post_id: PostId,
    pub location: GeoPoint,
    pub post_type: String,
    pub thumbnail_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub initial_score: f64,
}

impl IdentifiableCommand for IndexActivePostCommand {
    type Id = ProfileId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn cache_enabled(&self) -> bool {
        false
    }
}

impl IndexActivePostCommand {
    #[allow(clippy::too_many_arguments)]
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
        initial_score: f64,
    ) -> Self {
        Self {
            command_id,
            target: CommandTarget::stateless(author_id, region),
            post_id,
            location,
            post_type,
            thumbnail_url,
            created_at,
            expires_at,
            initial_score,
        }
    }
}
