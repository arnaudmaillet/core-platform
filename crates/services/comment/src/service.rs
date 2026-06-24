//! Adapts the comment composition root to the fleet [`service_runtime::Service`]
//! contract. Comment is ScyllaDB-only and publishes domain events through the
//! durable Kafka publisher.

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use scylla_storage::ScyllaConfig;
use service_runtime::{HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::{KafkaClientConfig, ProducerConfig};
use transport::kafka::producer::KafkaProducerBuilder;

use crate::app::{App, Backends};
use crate::infrastructure::grpc::handler::comment_service_handler::{
    CommentServiceHandler, CommentServiceServer,
};
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;
use crate::infrastructure::publisher::KafkaCommentEventPublisher;

type CommentServer =
    CommentServiceServer<CommentServiceHandler<Arc<InMemoryCommandBus>, Arc<InMemoryQueryBus>>>;

/// The comment service as hosted by [`service_runtime`].
pub struct CommentService {
    app: App,
}

#[async_trait]
impl Service for CommentService {
    const NAME: &'static str = "comment";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <CommentServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
        };

        let producer = KafkaProducerBuilder::new(ProducerConfig::new(KafkaClientConfig::from_env()))
            .build()?;
        let publisher = Arc::new(KafkaCommentEventPublisher::new(producer));

        let app = App::build(backends, publisher)
            .await
            .map_err(|e| anyhow::anyhow!("comment app build: {e}"))?;

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![scylla_storage::health::probe(Arc::clone(&self.app.scylla))]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let handler = CommentServiceHandler::new(
            Arc::clone(&self.app.command_bus),
            Arc::clone(&self.app.query_bus),
        );
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(CommentServiceServer::new(handler));
        Ok(())
    }
}
