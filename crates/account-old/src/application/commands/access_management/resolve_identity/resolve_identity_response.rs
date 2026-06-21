// crates/account/src/application/resolve_identity/dto.rs

use crate::domain::types::{AccountRole, AccountState};
use shared_kernel::types::AccountId;

#[derive(Debug, Clone)]
pub struct ResolvedIdentityResponse {
    pub account_id: AccountId,
    pub role: AccountRole,
    pub state: AccountState,
    pub is_beta_tester: bool,
}
