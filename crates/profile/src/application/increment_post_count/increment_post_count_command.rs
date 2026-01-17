// crates/profile/src/application/use_cases/increment_post_count/increment_post_count_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{RegionCode, AccountId, PostId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementPostCountCommand {
    pub account_id: AccountId,
    pub post_id: PostId,
    pub region: RegionCode,
}