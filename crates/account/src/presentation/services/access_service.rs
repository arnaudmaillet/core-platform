// crates/account/src/infrastructure/api/grpc/access_service.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use shared_kernel::command::CommandBus;
use shared_kernel::types::{AccountId, RegionCode, SubId};

use shared_proto::account::v1::account_access_service_server::AccountAccessService as ProtoAccountAccessService;
use shared_proto::account::v1::{
    LinkSubIdentityRequest, LinkSubIdentityResponse, RegisterRequest, RegisterResponse,
    ResolveIdentityRequest, ResolveIdentityResponse,
};

use crate::application::context::AccountAppContext;
use crate::commands::{LinkSubIdentityCommand, RegisterCommand};
use crate::presentation::utils::GrpcServiceUtils;

pub struct AccountAccessService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl AccountAccessService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for AccountAccessService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoAccountAccessService for AccountAccessService {
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let (_metadata, extensions, req) = request.into_parts();

        let region = RegionCode::try_new(&req.region_code)
            .map_err(|e| Status::invalid_argument(format!("Invalid region: {}", e)))?;

        let account_id = match &req.sub_id {
            Some(id) if !id.is_empty() => {
                let uuid = Uuid::parse_str(id)
                    .map_err(|_| Status::invalid_argument("Invalid sub_id UUID"))?;
                AccountId::from_external_uuid(uuid, region)
            }
            _ => AccountId::generate(region),
        };

        let mut command = RegisterCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        command.account_id = account_id;

        let ctx = self.build_creation_context(&extensions)?;

        let response_payload = RegisterResponse {
            account_id: account_id.uuid().to_string(),
        };

        self.dispatch_command::<RegisterCommand, (), RegisterResponse>(
            &ctx,
            command,
            response_payload,
        )
        .await
    }

    async fn link_sub_identity(
        &self,
        request: Request<LinkSubIdentityRequest>,
    ) -> Result<Response<LinkSubIdentityResponse>, Status> {
        let (_metadata, _extensions, req) = request.into_parts();

        let command = LinkSubIdentityCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&Request::new(()), command.target.id)?;

        let response_payload = LinkSubIdentityResponse {};

        self.dispatch_command::<LinkSubIdentityCommand, (), LinkSubIdentityResponse>(
            &ctx,
            command,
            response_payload,
        )
        .await
    }

    async fn resolve_identity(
        &self,
        request: Request<ResolveIdentityRequest>,
    ) -> Result<Response<ResolveIdentityResponse>, Status> {
        let req = request.into_inner();

        let sub_id =
            SubId::try_new(req.sub_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let account = self
            .app_ctx
            .account_repo()
            .find_by_sub_id(&sub_id, None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("No account associated with this sub identity"))?;

        Ok(Response::new(ResolveIdentityResponse {
            account_id: account.account_id().to_string(),
            state: *account.identity().state() as i32,
            role: account.governance().role() as i32,
        }))
    }
}
