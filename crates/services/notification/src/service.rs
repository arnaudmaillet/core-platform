//! Adapts the notification composition root to the fleet
//! [`service_runtime::Service`] contract.
//!
//! The handler is generic over its broadcast registry (for the streaming RPC) in
//! addition to the command/query buses; all three are the live instances `App`
//! exposes. Kafka workers are spawned inside [`App::build`].

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::KafkaClientConfig;

use crate::app::{App, Backends};
use crate::config::NotificationConfig;
use crate::infrastructure::grpc::handler::{NotificationServiceHandler, NotificationServiceServer};
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;
use crate::infrastructure::streaming::BroadcastRegistry;

type NotificationServer = NotificationServiceServer<
    NotificationServiceHandler<Arc<InMemoryCommandBus>, Arc<InMemoryQueryBus>, BroadcastRegistry>,
>;

/// The notification service as hosted by [`service_runtime`].
pub struct NotificationService {
    app: App,
}

#[async_trait]
impl Service for NotificationService {
    const NAME: &'static str = "notification";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str =
        <NotificationServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let config = Arc::new(NotificationConfig::from_env());
        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
            redis:  RedisConfig::from_env(),
            kafka:  Some(KafkaClientConfig::from_env()),
        };

        let app = App::build(config, backends)
            .await
            .map_err(|e| anyhow::anyhow!("notification app build: {e}"))?;

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
        let handler = NotificationServiceHandler::new(
            Arc::clone(&self.app.command_bus),
            Arc::clone(&self.app.query_bus),
            Arc::clone(&self.app.stream_registry),
        );
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(NotificationServiceServer::new(handler));
        Ok(())
    }
}
