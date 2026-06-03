use serde::{Deserialize, Serialize};

use crate::types::Region;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTarget<ID> {
    pub id: ID,
    pub region: Region,
    pub expected_version: Option<u64>,
}

impl<ID> CommandTarget<ID> {
    pub fn versioned(id: ID, region: Region, version: u64) -> Self {
        Self {
            id,
            region,
            expected_version: Some(version),
        }
    }

    pub fn stateless(id: ID, region: Region) -> Self {
        Self {
            id,
            region,
            expected_version: None,
        }
    }
}
