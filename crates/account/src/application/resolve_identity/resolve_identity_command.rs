// crates/account/src/application/resolve_identity/resolve_identity_command.rs

use crate::domain::value_objects::ExternalId;

#[derive(Debug, Clone)]
pub struct ResolveIdentityCommand {
    pub external_id: ExternalId,
}