// crates/account/src/infrastructure/api/grpc/settings_service.rs

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

pub struct AccountSettingsService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl AccountSettingsService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for AccountSettingsService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoAccountSettingsService for AccountSettingsService {
    async fn update_preferences(
        &self,
        request: Request<UpdatePreferencesRequest>,
    ) -> Result<Response<UpdatePreferencesResponse>, Status> {
        let command = UpdatePreferencesCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, command.target.id)?;

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
        let command = UpdateTimezoneCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, command.target.id)?;

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
        let command = AddPushTokenCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, command.target.id)?;

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
        let command = RemovePushTokenCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, command.target.id)?;

        self.dispatch_command::<RemovePushTokenCommand, (), RemovePushTokenResponse>(
            &ctx,
            command,
            RemovePushTokenResponse {},
        )
        .await
    }
}
