// crates/post/profile/src/domain/read_projection.rs
use serde::{Deserialize, Serialize};
use shared_kernel::types::ProfileId;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectedProfile {
    pub id: ProfileId,
    pub handle: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_verified: bool,
}
