use shared_kernel::core::TransactionManager;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_kernel::command::CommandBus;
use shared_kernel::types::{AccountId, SubId};

use shared_proto::account::v1::account_access_service_server::AccountAccessService as ProtoAccountAccessService;
use shared_proto::account::v1::{
    LinkSubIdentityRequest, LinkSubIdentityResponse, ResolveIdentityRequest,
    ResolveIdentityResponse,
};

use crate::application::context::AccountAppContext;
use crate::commands::LinkSubIdentityCommand;
use crate::presentation::utils::GrpcServiceUtils;

pub struct AccountAccessService<TM> {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext<TM>>,
}

impl<TM> AccountAccessService<TM> {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext<TM>>) -> Self {
        Self { bus, app_ctx }
    }
}

impl<TM: TransactionManager + Clone + 'static> GrpcServiceUtils<TM> for AccountAccessService<TM> {
    fn app_ctx(&self) -> &AccountAppContext<TM> {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl<TM: TransactionManager + Clone + 'static> ProtoAccountAccessService
    for AccountAccessService<TM>
{
    async fn link_sub_identity(
        &self,
        request: Request<LinkSubIdentityRequest>,
    ) -> Result<Response<LinkSubIdentityResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = LinkSubIdentityCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<LinkSubIdentityCommand, (), LinkSubIdentityResponse>(
            &ctx,
            command,
            LinkSubIdentityResponse {},
        )
        .await
    }

    async fn resolve_identity(
        &self,
        request: Request<ResolveIdentityRequest>,
    ) -> Result<Response<ResolveIdentityResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let sub_id =
            SubId::try_new(req.sub_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let query_ctx = self.build_query_context(&extensions)?;

        let account = self
            .app_ctx
            .account_repo()
            .find_by_sub_id(query_ctx.region(), &sub_id, None)
            .await
            .map_err(|e| {
                Status::internal(format!("Database error during identity resolution: {}", e))
            })?
            .ok_or_else(|| {
                Status::not_found(
                    "No account associated with this sub identity in the target region",
                )
            })?;

        Ok(Response::new(ResolveIdentityResponse {
            account_id: account.account_id().to_string(),
            state: account.identity().state().to_string(),
            role: account.governance().role().to_string(),
        }))
    }
}
