use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_settings_service_server::AccountSettingsService as ProtoAccountSettingsService;
use shared_proto::account::v1::{
    AddPushTokenRequest, AddPushTokenResponse, RemovePushTokenRequest, RemovePushTokenResponse,
    UpdatePreferencesRequest, UpdatePreferencesResponse, UpdateTimezoneRequest,
    UpdateTimezoneResponse,
};

use crate::application::context::AccountKernelCtx;
use crate::commands::{
    AddPushTokenCommand, RemovePushTokenCommand, UpdatePreferencesCommand, UpdateTimezoneCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

pub struct AccountSettingsService {
    bus: Arc<CommandBus>,
    kernel_ctx: AccountKernelCtx,
}

impl AccountSettingsService {
    pub fn new(bus: Arc<CommandBus>, kernel_ctx: AccountKernelCtx) -> Self {
        Self { bus, kernel_ctx }
    }
}

impl GrpcServiceUtils for AccountSettingsService {
    fn kernel_ctx(&self) -> &AccountKernelCtx {
        &self.kernel_ctx
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
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = UpdatePreferencesCommand::try_from_proto(req, ctx.region())
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
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = UpdateTimezoneCommand::try_from_proto(req, ctx.region())
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
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = AddPushTokenCommand::try_from_proto(req, ctx.region())
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
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = RemovePushTokenCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<RemovePushTokenCommand, (), RemovePushTokenResponse>(
            &ctx,
            command,
            RemovePushTokenResponse {},
        )
        .await
    }
}
