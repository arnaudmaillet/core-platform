// crates/account/src/test_utils/test_context_builder.rs

use crate::AccountTestContext;
use account::{
    AccountServiceBuilder,
    services::{
        AccountAccessService, AccountModerationService, AccountPersonalService,
        AccountSettingsService,
    },
};
use auth::{AuthInterceptor, KeycloakValidator};
use infra_test::{KeycloakTestContext, TestContextBuilder};
use shared_proto::account::v1::account_access_service_server::AccountAccessServiceServer;
use shared_proto::account::v1::account_moderation_service_server::AccountModerationServiceServer;
use shared_proto::account::v1::account_personal_service_server::AccountPersonalServiceServer;
use shared_proto::account::v1::account_settings_service_server::AccountSettingsServiceServer;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

pub struct AccountTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
}

impl AccountTestContextBuilder {
    pub fn new() -> Self {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let pg_migrations = manifest_dir.join("migrations/postgres");

        Self {
            kernel_builder: TestContextBuilder::new()
                .with_postgres(vec![pg_migrations])
                .with_redis(),
            with_grpc: false,
        }
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.with_grpc = true;
        self
    }

    pub async fn build(self) -> AccountTestContext {
        tracing::info!("Building Account test infrastructure...");
        let kernel_infra = self.kernel_builder.build().await;
        let pg_pool = kernel_infra.postgres().pool().clone();
        let redis_repo = kernel_infra.redis().repository();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting gRPC server...");
            let pg = pg_pool.clone();
            let redis = redis_repo.clone();

            tokio::spawn(async move {
                let auth_ctx = KeycloakTestContext::restore("master").await;
                let validator = Arc::new(
                    KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                        .await
                        .unwrap(),
                );
                let interceptor = AuthInterceptor::new(validator);

                let builder = AccountServiceBuilder::new(pg, redis);
                let app_ctx = builder.build_context();
                let bus = builder.build_command_bus();

                let svc = Server::builder()
                    .add_service(AccountAccessServiceServer::with_interceptor(
                        AccountAccessService::new(bus.clone(), app_ctx.clone()),
                        interceptor.clone(),
                    ))
                    .add_service(AccountModerationServiceServer::with_interceptor(
                        AccountModerationService::new(bus.clone(), app_ctx.clone()),
                        interceptor.clone(),
                    ))
                    .add_service(AccountPersonalServiceServer::with_interceptor(
                        AccountPersonalService::new(bus.clone(), app_ctx.clone()),
                        interceptor.clone(),
                    ))
                    .add_service(AccountSettingsServiceServer::with_interceptor(
                        AccountSettingsService::new(bus, app_ctx),
                        interceptor,
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
