//! Adapts the geo-discovery composition root to the fleet
//! [`service_runtime::Service`] contract.
//!
//! Geo-discovery's gRPC surface is query-only (writes arrive via Kafka workers
//! spawned inside [`App::build`]), so the handler is constructed from the query
//! bus alone.

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::query::InMemoryQueryBus;
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::KafkaClientConfig;

use crate::app::{App, Backends};
use crate::config::GeoDiscoveryConfig;
use crate::infrastructure::grpc::handler::{GeoDiscoveryHandler, GeoDiscoveryServiceServer};
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;

type GeoServer = GeoDiscoveryServiceServer<GeoDiscoveryHandler<Arc<InMemoryQueryBus>>>;

/// The geo-discovery service as hosted by [`service_runtime`].
pub struct GeoDiscoveryService {
    app: App,
}

#[async_trait]
impl Service for GeoDiscoveryService {
    const NAME: &'static str = "geo-discovery";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <GeoServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let cfg = GeoDiscoveryConfig::from_env();
        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
            redis:  RedisConfig::from_env(),
            kafka:  Some(KafkaClientConfig::from_env()),
        };

        let app = App::build(cfg, backends)
            .await
            .map_err(|e| anyhow::anyhow!("geo-discovery app build: {e}"))?;

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        let scylla = Arc::clone(&self.app.scylla);
        let redis = self.app.redis.clone();
        vec![
            Arc::new(FnProbe::new("scylla", move || {
                let scylla = Arc::clone(&scylla);
                async move {
                    scylla_storage::health::health_check(&scylla.session)
                        .await
                        .map_err(|e| anyhow::anyhow!("scylla: {e}"))
                }
            })),
            Arc::new(FnProbe::new("redis", move || {
                let redis = redis.clone();
                async move {
                    redis_storage::health::health_check(&*redis)
                        .await
                        .map_err(|e| anyhow::anyhow!("redis: {e}"))
                }
            })),
        ]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let handler = GeoDiscoveryHandler::new(Arc::clone(&self.app.query_bus));
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(GeoDiscoveryServiceServer::new(handler));
        Ok(())
    }
}
