// crates/account/src/application/resolve_identity/resolve_identity_command.rs

// use shared_proto::account::v1::ResolveIdentityRequest;

use uuid::Uuid;

use crate::domain::types::SubId;

#[derive(Debug, Clone)]
pub struct ResolveIdentityCommand {
    pub command_id: Uuid,
    pub sub_id: SubId,
}

impl ResolveIdentityCommand {
    pub fn try_from_proto(req: ResolveIdentityRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            sub_id: SubId::try_from(req.sub_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid SubId: {}", e))
            })?,
        })
    }
}
