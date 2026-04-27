use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::domain::value_objects::Timezone;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTimezoneCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_timezone: Timezone,
}
