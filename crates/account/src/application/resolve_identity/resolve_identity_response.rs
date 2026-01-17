// crates/account/src/application/resolve_identity/dto.rs

use shared_kernel::domain::value_objects::AccountId;
use crate::domain::value_objects::{AccountRole, AccountState};

#[derive(Debug, Clone)]
pub struct ResolvedIdentityResponse {
    pub account_id: AccountId,
    pub role: AccountRole,
    pub state: AccountState,
    pub is_beta_tester: bool,
}