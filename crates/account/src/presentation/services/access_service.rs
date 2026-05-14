// crates/account/src/infrastructure/api/grpc/access_service.rs

use shared_kernel::core::AggregateRoot;
use shared_kernel::types::{AccountId, RegionCode, SubId};
use uuid::Uuid;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_access_service_server::AccountAccessService as ProtoAccountAccessService;
use shared_proto::account::v1::{
    AccountIdentity, LinkSubIdentityRequest, RegisterRequest, ResolveIdentityRequest,
    ResolveIdentityResponse,
};

use crate::application::context::AccountAppContext;

use crate::commands::{LinkSubIdentityCommand, RegisterCommand};
use crate::presentation::utils::{GrpcServiceUtils, map_account_to_identity_proto};
use shared_kernel::command::CommandBus;

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
    ) -> Result<Response<AccountIdentity>, Status> {
        let req = request.into_inner();

        // 1. Détermination de l'ID (Généré ou mappé depuis le sub_id)
        let region = RegionCode::try_new(req.region_code.clone())
            .map_err(|e| Status::invalid_argument(format!("Invalid region: {}", e)))?;

        let account_id = match &req.sub_id {
            Some(id) if !id.is_empty() => {
                let uuid = Uuid::parse_str(id)
                    .map_err(|_| Status::invalid_argument("Invalid sub_id UUID"))?;
                AccountId::new(uuid, region)
            }
            _ => AccountId::generate(region),
        };

        // 2. Création de la commande (on retire account_id de l'argument si tu as mis à jour try_from_proto)
        let command = RegisterCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 3. Création du contexte avec l'ID qui va être créé
        let ctx = self.app_ctx.create_context(account_id);

        // 4. Exécution (Le handler renvoie l'AccountId créé)
        self.execute_and_fetch::<RegisterCommand, (), AccountIdentity, _>(
            &ctx,
            command,
            map_account_to_identity_proto,
        )
        .await
    }

    async fn link_sub_identity(
        &self,
        request: Request<LinkSubIdentityRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let command = LinkSubIdentityCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Pattern standard : command.target.id
        let ctx = self.get_context(&request, &command.target.id)?;

        self.execute_and_fetch::<LinkSubIdentityCommand, (), AccountIdentity, _>(
            &ctx,
            command,
            map_account_to_identity_proto,
        )
        .await
    }

    async fn resolve_identity(
        &self,
        request: Request<ResolveIdentityRequest>,
    ) -> Result<Response<ResolveIdentityResponse>, Status> {
        let req = request.into_inner();

        // Correction de la lecture du sub_id (ValueObject SubId)
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
            account_id: account.id().to_string(),
            state: *account.identity().state() as i32,
            role: account.governance().role() as i32,
        }))
    }
}
