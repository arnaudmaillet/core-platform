use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAvatarCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_avatar_url: Url,
}