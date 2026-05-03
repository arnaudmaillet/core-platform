// crates/account/src/infrastructure/api/grpc/access_service.rs

use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::AccountId;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_access_service_server::AccountAccessService;
use shared_proto::account::v1::{
    AccountIdentity, LinkSubIdentityRequest, RegisterRequest, ResolveIdentityRequest,
    ResolveIdentityResponse,
};

use crate::application::context::AccountAppContext;

use crate::infrastructure::api::grpc::mapper;
use crate::infrastructure::api::grpc::shared::GrpcServiceUtils;
use crate::use_cases::{LinkSubIdentityCommand, RegisterCommand};
use shared_kernel::application::CommandBus;

pub struct GrpcAccessService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl GrpcAccessService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for GrpcAccessService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl AccountAccessService for GrpcAccessService {
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let req = request.into_inner();

        let account_id = match &req.sub_id {
            Some(id) if !id.is_empty() => AccountId::try_new(id.clone())
                .map_err(|e| Status::invalid_argument(e.to_string()))?,
            _ => AccountId::new(),
        };

        let command = RegisterCommand::try_from_proto(req, account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Le contexte utilisera maintenant l'ID stable
        let ctx = self
            .app_ctx
            .create_context(command.account_id.clone(), command.region.clone());

        self.execute_and_fetch::<RegisterCommand, AccountId, AccountIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn link_sub_identity(
        &self,
        request: Request<LinkSubIdentityRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let command = LinkSubIdentityCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<LinkSubIdentityCommand, (), AccountIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn resolve_identity(
        &self,
        request: Request<ResolveIdentityRequest>,
    ) -> Result<Response<ResolveIdentityResponse>, Status> {
        let req = request.into_inner();
        let sub_id = req
            .sub_id
            .parse()
            .map_err(|_| Status::invalid_argument("Invalid sub_id"))?;

        // Query directe sur le repo (lecture seule, pas besoin de bus)
        let account = self
            .app_ctx
            .account_repo()
            .find_by_sub_id(&sub_id, None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("No account associated with this sub identity"))?;

        Ok(Response::new(ResolveIdentityResponse {
            account_id: account.id(),
            state: *account.identity().state() as i32,
            role: account.governance().role() as i32,
        }))
    }
}
