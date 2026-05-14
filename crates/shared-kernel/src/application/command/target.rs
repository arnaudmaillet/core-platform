use serde::{Deserialize, Serialize};

use crate::types::RegionCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTarget<ID> {
    pub id: ID,
    pub region: RegionCode,
    pub expected_version: u64,
}

impl<ID> CommandTarget<ID> {
    pub fn new(id: ID, region: RegionCode, expected_version: u64) -> Self {
        Self {
            id,
            region,
            expected_version,
        }
    }
}
