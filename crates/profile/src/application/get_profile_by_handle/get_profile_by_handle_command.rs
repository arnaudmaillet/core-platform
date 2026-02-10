// crates/profile/src/application/queries/get_profile_by_handle/get_profile_by_handle_command

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::RegionCode;
use crate::domain::value_objects::Handle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProfileByHandleCommand {
    pub handle: Handle,
    pub region: RegionCode,
}
