// crates/post/src/infrastructure/media/scylla/models.rs

use infra_scylla::scylla_macros::{DeserializeValue, SerializeValue};
use uuid::Uuid;

#[derive(Debug, Clone, DeserializeValue, SerializeValue)]
#[scylla(crate = "infra_scylla::scylla")]
pub struct ScyllaMediaModel {
    pub media_id: Uuid,
    pub url: String,
    pub thumbnail_url: String,
    pub duration_seconds: i32,
    pub width: i32,
    pub height: i32,
    pub media_type: String,
    pub mime_type: String,
}
