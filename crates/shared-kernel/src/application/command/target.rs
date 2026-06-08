// shared_kernel/src/command.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTarget<ID> {
    pub id: ID,
    pub expected_version: Option<u64>,
}

impl<ID> CommandTarget<ID> {
    pub fn versioned(id: ID, version: u64) -> Self {
        Self {
            id,
            expected_version: Some(version),
        }
    }

    pub fn stateless(id: ID) -> Self {
        Self {
            id,
            expected_version: None,
        }
    }
}
