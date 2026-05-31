use shared_kernel::core::TransactionManager;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_settings_service_server::AccountSettingsService as ProtoAccountSettingsService;
use shared_proto::account::v1::{
    AddPushTokenRequest, AddPushTokenResponse, RemovePushTokenRequest, RemovePushTokenResponse,
    UpdatePreferencesRequest, UpdatePreferencesResponse, UpdateTimezoneRequest,
    UpdateTimezoneResponse,
};

use crate::application::context::AccountAppContext;
use crate::commands::{
    AddPushTokenCommand, RemovePushTokenCommand, UpdatePreferencesCommand, UpdateTimezoneCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

pub struct AccountSettingsService<TM> {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext<TM>>,
}

impl<TM> AccountSettingsService<TM> {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext<TM>>) -> Self {
        Self { bus, app_ctx }
    }
}

impl<TM: TransactionManager + Clone + 'static> GrpcServiceUtils<TM> for AccountSettingsService<TM> {
    fn app_ctx(&self) -> &AccountAppContext<TM> {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl<TM: TransactionManager + Clone + 'static> ProtoAccountSettingsService
    for AccountSettingsService<TM>
{
    async fn update_preferences(
        &self,
        request: Request<UpdatePreferencesRequest>,
    ) -> Result<Response<UpdatePreferencesResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = UpdatePreferencesCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdatePreferencesCommand, (), UpdatePreferencesResponse>(
            &ctx,
            command,
            UpdatePreferencesResponse {},
        )
        .await
    }

    async fn update_timezone(
        &self,
        request: Request<UpdateTimezoneRequest>,
    ) -> Result<Response<UpdateTimezoneResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = UpdateTimezoneCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateTimezoneCommand, (), UpdateTimezoneResponse>(
            &ctx,
            command,
            UpdateTimezoneResponse {},
        )
        .await
    }

    async fn add_push_token(
        &self,
        request: Request<AddPushTokenRequest>,
    ) -> Result<Response<AddPushTokenResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = AddPushTokenCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<AddPushTokenCommand, (), AddPushTokenResponse>(
            &ctx,
            command,
            AddPushTokenResponse {},
        )
        .await
    }

    async fn remove_push_token(
        &self,
        request: Request<RemovePushTokenRequest>,
    ) -> Result<Response<RemovePushTokenResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = RemovePushTokenCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<RemovePushTokenCommand, (), RemovePushTokenResponse>(
            &ctx,
            command,
            RemovePushTokenResponse {},
        )
        .await
    }
}
