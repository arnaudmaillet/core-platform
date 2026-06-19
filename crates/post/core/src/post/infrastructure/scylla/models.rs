// crates/post/src/infrastructure/post/scylla/models.rs

use crate::media::ScyllaMediaModel;
use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, scylla::DeserializeRow, Clone)]
pub struct ScyllaPostModel {
    pub author_id: Uuid,
    pub post_id: Uuid,
    pub post_type: String,
    pub caption: Option<String>,
    pub media_list: Vec<ScyllaMediaModel>,
    pub total_duration_seconds: i32,
    pub allowed_comment_hands: bool,
    pub visibility_level: String,
    pub music_id: Option<Uuid>,
    pub hashtags: HashSet<String>,
    pub mentions: HashSet<Uuid>,
    pub edited_at: Option<CqlTimestamp>,
    pub created_at: Option<CqlTimestamp>,
    pub dynamic_metadata: String,
}
