//! Adapts the account composition root to the fleet [`service_runtime::Service`]
//! contract. Account is PostgreSQL-backed; the pool is built here, shared into
//! [`App::build`], and reused (it is `Clone`/`Arc`-backed) for the readiness probe.

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use postgres_storage::{PgPoolBuilder, PostgresConfig};
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use sqlx::PgPool;
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;

use crate::app::App;
use crate::infrastructure::grpc::handler::account_service_handler::AccountServiceServer;
use crate::infrastructure::grpc::handler::AccountServiceHandler;
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;

type AccountServer =
    AccountServiceServer<AccountServiceHandler<Arc<InMemoryCommandBus>, Arc<InMemoryQueryBus>>>;

/// The account service as hosted by [`service_runtime`].
pub struct AccountService {
    app: App,
    pool: PgPool,
}

#[async_trait]
impl Service for AccountService {
    const NAME: &'static str = "account";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <AccountServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let pool = PgPoolBuilder::build(PostgresConfig::from_env())
            .await
            .map_err(|e| anyhow::anyhow!("account postgres pool: {e}"))?;

        // `PgPool` is `Arc`-backed: one clone serves the app graph, one the probe.
        let app = App::build(pool.clone())
            .await
            .map_err(|e| anyhow::anyhow!("account app build: {e}"))?;

        Ok(Self { app, pool })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        let pool = self.pool.clone();
        vec![Arc::new(FnProbe::new("postgres", move || {
            let pool = pool.clone();
            async move {
                postgres_storage::health_check(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("postgres: {e}"))
            }
        }))]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let handler = AccountServiceHandler::new(
            Arc::clone(&self.app.command_bus),
            Arc::clone(&self.app.query_bus),
        );
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(AccountServiceServer::new(handler));
        Ok(())
    }
}
