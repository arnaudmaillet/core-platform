// shared_kernel/src/types/routing.rs

use crate::types::Region;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    Regional(Region),
    Global,
}
