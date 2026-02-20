// crates/profile/src/application/use_cases/update_stats/update_stats_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::RegionCode;
use crate::domain::value_objects::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatsCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub follower_delta: i64, // ex: +5 si 5 personnes ont follow dans le batch
    pub following_delta: i64, // ex: -1 si une personne s'est désabonnée
    pub post_delta: i64,
}
