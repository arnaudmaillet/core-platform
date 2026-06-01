// crates/account-test-utils/src/test_context_builder.rs

use crate::AccountTestContext;
use account::{
    AccountServiceBuilder,
    services::{
        AccountAccessService, AccountModerationService, AccountPersonalService,
        AccountRegistrationService, AccountSettingsService,
    },
};
use auth::{
    TokenValidator,
    interceptors::{AuthInterceptor, RegistrationInterceptor},
};
use auth_test_utils::KeycloakTestContext;
use infra_test::TestContextBuilder;
use shared_proto::account::v1::account_access_service_server::AccountAccessServiceServer;
use shared_proto::account::v1::account_moderation_service_server::AccountModerationServiceServer;
use shared_proto::account::v1::account_personal_service_server::AccountPersonalServiceServer;
use shared_proto::account::v1::account_registration_service_server::AccountRegistrationServiceServer;
use shared_proto::account::v1::account_settings_service_server::AccountSettingsServiceServer;
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tonic::transport::Server;

pub struct AccountTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
    mock_validator: Option<Arc<dyn TokenValidator>>,
}

impl AccountTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new()
                .with_postgres(vec!["crates/account/migrations/postgres"])
                .with_redis(),
            with_grpc: false,
            mock_validator: None,
        }
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.with_grpc = true;
        self
    }

    pub fn with_mock_auth(mut self, validator: Arc<dyn TokenValidator>) -> Self {
        self.mock_validator = Some(validator);
        self
    }

    pub async fn build(self) -> AccountTestContext {
        tracing::info!("Building Account test infrastructure...");
        let kernel_infra = self.kernel_builder.build().await;
        let pg_pool = kernel_infra.postgres().pool().clone();
        let global_pg_pool = kernel_infra.postgres().pool().clone();

        let redis_repo = kernel_infra.redis().repository();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting gRPC server...");
            let pg = pg_pool.clone();
            let global_pg = global_pg_pool.clone();
            let redis = redis_repo.clone();
            let custom_validator = self.mock_validator.clone();
            let otp_ttl = Duration::from_secs(60 * 15);

            tokio::spawn(async move {
                let validator = match custom_validator {
                    Some(mock) => mock,
                    None => {
                        let auth_ctx = KeycloakTestContext::restore(
                            "master",
                            "account-service-test".to_string(),
                        )
                        .await;
                        auth_ctx.validator.clone()
                    }
                };

                let auth_interceptor = AuthInterceptor::new(validator.clone());
                let registration_interceptor = RegistrationInterceptor::new(validator);

                let builder = AccountServiceBuilder::new(pg, global_pg, redis, otp_ttl);
                let app_ctx = builder.build_context();
                let bus = builder.build_command_bus();

                let svc = Server::builder()
                    .add_service(AccountRegistrationServiceServer::with_interceptor(
                        AccountRegistrationService::new(bus.clone(), app_ctx.clone()),
                        registration_interceptor,
                    ))
                    .add_service(AccountAccessServiceServer::with_interceptor(
                        AccountAccessService::new(bus.clone(), app_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountModerationServiceServer::with_interceptor(
                        AccountModerationService::new(bus.clone(), app_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountPersonalServiceServer::with_interceptor(
                        AccountPersonalService::new(bus.clone(), app_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountSettingsServiceServer::with_interceptor(
                        AccountSettingsService::new(bus, app_ctx),
                        auth_interceptor,
                    ));

                let addr = "[::1]:0".parse::<std::net::SocketAddr>().unwrap();
                let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                let actual_addr = listener.local_addr().unwrap();
                let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                tracing::info!(port = %actual_addr.port(), "Account gRPC server listening");
                ready_tx.send(actual_addr).ok();

                svc.serve_with_incoming_shutdown(incoming, async {
                    shutdown_rx.await.ok();
                    tracing::info!("Account gRPC server shutting down");
                })
                .await
                .unwrap();
            });
        }

        let addr = if self.with_grpc {
            Some(ready_rx.await.expect("Failed to start gRPC server"))
        } else {
            None
        };

        tracing::info!("Account infrastructure ready");
        AccountTestContext::new(kernel_infra, addr, Some(shutdown_tx))
    }
}
