//! Adapts the timeline composition root to the fleet [`service_runtime::Service`]
//! contract.
//!
//! Timeline depends on social-graph over gRPC: the concrete
//! [`SocialGraphGrpcClient`] is built here from the configured endpoint (lazily
//! connected, so boot doesn't block on the dependency) and injected into the
//! otherwise client-generic [`App::build`]. The gRPC surface is query-only;
//! ingestion runs via Kafka workers spawned inside `App::build`.

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::query::InMemoryQueryBus;
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic::transport::Channel;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::KafkaClientConfig;

use crate::app::{App, AppConfig, Backends};
use crate::config::TimelineConfig;
use crate::infrastructure::client::SocialGraphGrpcClient;
use crate::infrastructure::grpc::handler::{TimelineServiceHandler, TimelineServiceServer};
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;

type TimelineServer = TimelineServiceServer<TimelineServiceHandler<Arc<InMemoryQueryBus>>>;

/// The timeline service as hosted by [`service_runtime`].
pub struct TimelineService {
    app: App,
}

#[async_trait]
impl Service for TimelineService {
    const NAME: &'static str = "timeline";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <TimelineServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let cfg = TimelineConfig::from_env();

        let app_config = AppConfig {
            feed_cap:               cfg.feed_cap,
            audio_feed_cap:         cfg.audio_feed_cap,
            vip_registry_cap:       cfg.vip_registry_cap,
            backfill_limit:         cfg.backfill_limit,
            warm_ttl_secs:          cfg.warm_ttl_secs,
            tier_cache_ttl_secs:    cfg.tier_cache_ttl_secs,
            vip_registry_ttl_secs:  cfg.vip_registry_ttl_secs,
            max_page_size:          cfg.max_page_size,
            max_vip_merge_sources:  cfg.max_vip_merge_sources,
            warm_max_concurrency:   cfg.warm_max_concurrency,
            social_graph_page_size: cfg.social_graph_page_size,
            kafka_group_post_published: cfg.kafka_group_post_published.clone(),
            kafka_group_post_deleted:   cfg.kafka_group_post_deleted.clone(),
            kafka_group_sg_followed:    cfg.kafka_group_sg_followed.clone(),
            kafka_group_sg_unfollowed:  cfg.kafka_group_sg_unfollowed.clone(),
        };

        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
            redis:  RedisConfig::from_env(),
            kafka:  Some(KafkaClientConfig::from_env()),
        };

        // Lazily-connected channel: timeline boots even if social-graph is not
        // yet reachable; the worker/cold-rebuild call sites tolerate latency.
        let channel = Channel::from_shared(cfg.social_graph_endpoint.clone())
            .map_err(|e| anyhow::anyhow!("invalid social-graph endpoint: {e}"))?
            .connect_lazy();
        let social_graph = Arc::new(SocialGraphGrpcClient::new(channel));

        let app = App::build(&app_config, backends, social_graph)
            .await
            .map_err(|e| anyhow::anyhow!("timeline app build: {e}"))?;

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
        let handler = TimelineServiceHandler::new(Arc::clone(&self.app.query_bus));
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(TimelineServiceServer::new(handler));
        Ok(())
    }
}
