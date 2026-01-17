// crates/profile/src/application/use_cases/decrement_post_count/decrement_post_count_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecrementPostCountCommand {
    pub account_id: AccountId,
    pub post_id: PostId,
    pub region: RegionCode,
}