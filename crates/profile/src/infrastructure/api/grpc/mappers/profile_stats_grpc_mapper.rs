// Dans crates/profile/src/infrastructure/api/grpc/mappers/profile_stats_grpc_mapper.rs

use super::super::profile_v1::ProfileStats as ProtoProfileStats;
use crate::domain::value_objects::ProfileStats;
use shared_kernel::errors::{DomainError, Result};

impl From<ProfileStats> for ProtoProfileStats {
    fn from(domain: ProfileStats) -> Self {
        Self {
            follower_count: domain.follower_count() as i64,
            following_count: domain.following_count() as i64,
        }
    }
}

impl TryFrom<ProtoProfileStats> for ProfileStats {
    type Error = DomainError;

    fn try_from(proto: ProtoProfileStats) -> Result<Self> {
        Ok(Self::new(
            proto.follower_count.max(0) as u64,
            proto.following_count.max(0) as u64,
        ))
    }
}
