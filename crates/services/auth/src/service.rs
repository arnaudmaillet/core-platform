//! Adapts the auth composition root to the fleet [`service_runtime::Service`]
//! contract. Maps env → config, defers to [`App::build`], registers the concrete
//! tonic service, and reports Postgres + Redis liveness via the storage crates'
//! ready-made probes over the connections `App` retains.

use std::sync::Arc;

use async_trait::async_trait;
use postgres_storage::PostgresConfig;
use redis_storage::RedisConfig;
use service_runtime::{HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::client::KafkaClientConfig;

use crate::app::{App, Backends};
use crate::config::AuthConfig;
use crate::infrastructure::grpc::handler::{AuthServiceHandler, AuthServiceServer};
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;

/// The concrete tonic server type, named once so the health key and the
/// reflection registration agree.
type AuthServer = AuthServiceServer<AuthServiceHandler>;

/// The auth service as hosted by [`service_runtime`].
pub struct AuthService {
    app: App,
}

#[async_trait]
impl Service for AuthService {
    const NAME: &'static str = "auth";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <AuthServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let config = AuthConfig::from_env()?;
        let backends = Backends {
            postgres: PostgresConfig::from_env(),
            redis: RedisConfig::from_env(),
            kafka: Some(KafkaClientConfig::from_env()),
        };

        // `App::build` errors are `Box<dyn Error>` (not `Send + Sync`); flatten to
        // a message rather than propagating the box into `anyhow`.
        let app = App::build(config, backends)
            .await
            .map_err(|e| anyhow::anyhow!("auth app build: {e}"))?;

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![
            postgres_storage::health::probe(self.app.pool.clone()),
            redis_storage::health::probe(self.app.redis.clone()),
        ]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(AuthServiceServer::new(self.app.handler));
        Ok(())
    }
}
