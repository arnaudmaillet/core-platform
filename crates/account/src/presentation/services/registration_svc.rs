// crates/account/src/presentation/grpc/registration_service.rs

use shared_kernel::core::TransactionManager;
use shared_proto::account::v1::registration_identifier::Method as ProtoMethod;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

use auth::domain::claims::Claims;
use shared_proto::account::v1::account_registration_service_server::AccountRegistrationService as ProtoAccountRegistrationService;
use shared_proto::account::v1::{
    RegisterRequest, RegisterResponse, RegistrationIdentifier as ProtoRegistrationIdentifier,
};

use crate::application::context::AccountAppContext;
use crate::commands::RegisterCommand;
use crate::presentation::utils::GrpcServiceUtils;

pub struct AccountRegistrationService<TM> {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext<TM>>,
}

impl<TM> AccountRegistrationService<TM> {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext<TM>>) -> Self {
        Self { bus, app_ctx }
    }
}

impl<TM: TransactionManager + Clone + 'static> GrpcServiceUtils<TM>
    for AccountRegistrationService<TM>
{
    fn app_ctx(&self) -> &AccountAppContext<TM> {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl<TM: TransactionManager + Clone + 'static> ProtoAccountRegistrationService
    for AccountRegistrationService<TM>
{
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let (_, extensions, mut req) = request.into_parts();

        // Détection du flux : Est-ce une inscription Sociale (Google/Facebook) pré-authentifiée ?
        // L'intercepteur public a validé le token et injecté les Claims si présents.
        if let Some(claims) = extensions.get::<Claims>() {
            req.sub_id = Some(claims.sub_id.to_string());

            if let Some(ref email) = claims.email {
                req.identifier = Some(ProtoRegistrationIdentifier {
                    method: Some(ProtoMethod::Email(email.as_str().to_string())),
                });
            }
        }

        let generated_account_id = AccountId::generate();
        let ctx = self.build_creation_context(&extensions)?;

        let command = RegisterCommand::try_from_proto(req, generated_account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let response_payload = RegisterResponse {
            account_id: generated_account_id.to_string(),
        };

        self.dispatch_command::<RegisterCommand, (), RegisterResponse>(
            &ctx,
            command,
            response_payload,
        )
        .await
        .map_err(|e| {
            tracing::error!(target: "account_debug", error = ?e, "Failed to execute registration pipeline");
            e
        })
    }
}
