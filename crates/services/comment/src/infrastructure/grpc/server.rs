use std::net::SocketAddr;
use std::sync::Arc;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{QueryBusBuilder, InMemoryQueryBus};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::producer::ProducerConfig;
use transport::kafka::producer::builder::KafkaProducerBuilder;

use crate::application::command::{
    create_comment::CreateCommentHandler,
    delete_comment::DeleteCommentHandler,
};
use crate::application::query::{
    get_comment::GetCommentHandler,
    list_replies::ListRepliesHandler,
    list_top_level::ListTopLevelHandler,
};
use crate::infrastructure::grpc::handler::comment_service_handler::{
    CommentServiceHandler, CommentServiceServer,
};
use crate::infrastructure::persistence::ScyllaCommentRepository;
use crate::infrastructure::publisher::KafkaCommentEventPublisher;

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("comment_descriptor");

/// Bootstraps and runs the comment gRPC server.
///
/// Initialises the ScyllaDB client, Kafka producer, and all CQRS buses, then
/// blocks until the server shuts down.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    // ── Storage ───────────────────────────────────────────────────────────────

    let scylla_config = ScyllaConfig::from_env();
    let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla_config).build().await?);

    // ── Kafka producer ────────────────────────────────────────────────────────

    let kafka_client    = KafkaClientConfig::from_env();
    let producer_config = ProducerConfig::new(kafka_client);
    let producer        = KafkaProducerBuilder::new(producer_config).build()?;

    // ── Infrastructure objects ────────────────────────────────────────────────

    let repository = Arc::new(ScyllaCommentRepository::new(Arc::clone(&scylla_client)));
    let publisher  = Arc::new(KafkaCommentEventPublisher::new(producer));

    // ── CQRS buses ────────────────────────────────────────────────────────────

    let command_bus = CommandBusBuilder::new()
        .register::<crate::application::command::create_comment::CreateCommentCommand, _>(
            CreateCommentHandler {
                repository: Arc::clone(&repository),
                publisher:  Arc::clone(&publisher),
            },
        )?
        .register::<crate::application::command::delete_comment::DeleteCommentCommand, _>(
            DeleteCommentHandler {
                repository: Arc::clone(&repository),
                publisher:  Arc::clone(&publisher),
            },
        )?
        .build();

    let query_bus = QueryBusBuilder::new()
        .register::<crate::application::query::get_comment::GetCommentQuery, _>(
            GetCommentHandler { repository: Arc::clone(&repository) },
        )?
        .register::<crate::application::query::list_top_level::ListTopLevelQuery, _>(
            ListTopLevelHandler { repository: Arc::clone(&repository) },
        )?
        .register::<crate::application::query::list_replies::ListRepliesQuery, _>(
            ListRepliesHandler { repository: Arc::clone(&repository) },
        )?
        .build();

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<CommentServiceServer<
            CommentServiceHandler<InMemoryCommandBus, InMemoryQueryBus>,
        >>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = CommentServiceServer::new(CommentServiceHandler::new(command_bus, query_bus));

    tracing::info!(addr = %addr, "comment gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
