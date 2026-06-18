// crates/post/src/infrastructure/redis/mappers/profile.rs

use shared_kernel::core::{Error, Result};
use shared_proto::profile::v1::ProfileSummaryDto;

pub struct RedisProfileModel;

impl RedisProfileModel {
    pub fn to_redis_value(dto: &ProfileSummaryDto) -> Result<String> {
        serde_json::to_string(dto)
            .map_err(|e| Error::internal(format!("Redis profile serialization failed: {}", e)))
    }

    pub fn from_redis_value(value: &str) -> Result<ProfileSummaryDto> {
        serde_json::from_str(value)
            .map_err(|e| Error::internal(format!("Redis profile deserialization failed: {}", e)))
    }
}
