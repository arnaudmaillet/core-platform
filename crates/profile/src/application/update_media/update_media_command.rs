// crates/profile/src/application/use_cases/update_media/update_media_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{RegionCode, Url, AccountId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMediaCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub avatar_url: Option<Option<Url>>,
    pub banner_url: Option<Option<Url>>,
}