// crates/account-test-utils/src/test_context_builder.rs

use crate::AccountTestContext;
use account_old::{
    AccountServiceBuilder,
    db::{PostgresAccountRepository, PostgresGlobalIdentityRegistry},
    fred::FredOtpRepository,
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
use infra_sqlx::{
    PostgresIdempotencyRepository, PostgresOutboxRepository, PostgresTransactionManager,
};
use infra_test::TestContextBuilder;
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::Region;
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
    cluster_ctx: ClusterContext,
}

impl AccountTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new()
                .with_postgres(vec!["crates/account/migrations/postgres"])
                .with_redis(),
            with_grpc: false,
            mock_validator: None,
            cluster_ctx: ClusterContext::new(Region::default()),
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

    pub fn with_cluster_ctx(mut self, ctx: ClusterContext) -> Self {
        self.cluster_ctx = ctx;
        self
    }

    pub async fn build(self) -> AccountTestContext {
        tracing::info!("Building Account test infrastructure...");
        let kernel_infra = self.kernel_builder.build().await;
        let pg_pool = kernel_infra.postgres().pool().clone();
        let fred_cache_owned = (*kernel_infra.redis().cache()).clone();
        let fred_idempotency_owned = (*kernel_infra.redis().idempotency()).clone();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting gRPC server...");
            let pg = pg_pool.clone();

            let cache_for_bus = fred_cache_owned.clone();
            let idempotency_for_bus = fred_idempotency_owned.clone();
            let cache_for_otp = fred_cache_owned;

            let custom_validator = self.mock_validator.clone();
            let cluster_ctx = self.cluster_ctx;
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
                let mut command_bus = CommandBus::new(
                    Some(Arc::new(idempotency_for_bus)),
                    Some(Arc::new(cache_for_bus)),
                );

                let account_repo = Arc::new(PostgresAccountRepository::new(pg.clone()));
                let outbox_repo = Arc::new(PostgresOutboxRepository::new(pg.clone()));
                let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new_with_pool(
                    pg.clone(),
                    "account",
                ));
                let global_registry = Arc::new(PostgresGlobalIdentityRegistry::new(pg.clone()));

                let otp_repo = Arc::new(FredOtpRepository::new(cache_for_otp, otp_ttl));
                let tx_manager = Arc::new(PostgresTransactionManager::new(pg));

                let builder = AccountServiceBuilder::new(
                    account_repo,
                    outbox_repo,
                    idempotency_repo,
                    global_registry,
                    otp_repo,
                    tx_manager,
                    cluster_ctx,
                );

                let kernel_ctx = builder.build_kernel_ctx();
                builder.register_handlers(&mut command_bus);
                let shared_bus = Arc::new(command_bus);

                let svc = Server::builder()
                    .add_service(AccountRegistrationServiceServer::with_interceptor(
                        AccountRegistrationService::new(shared_bus.clone(), kernel_ctx.clone()),
                        registration_interceptor,
                    ))
                    .add_service(AccountAccessServiceServer::with_interceptor(
                        AccountAccessService::new(shared_bus.clone(), kernel_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountModerationServiceServer::with_interceptor(
                        AccountModerationService::new(shared_bus.clone(), kernel_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountPersonalServiceServer::with_interceptor(
                        AccountPersonalService::new(shared_bus.clone(), kernel_ctx.clone()),
                        auth_interceptor.clone(),
                    ))
                    .add_service(AccountSettingsServiceServer::with_interceptor(
                        AccountSettingsService::new(shared_bus, kernel_ctx),
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
