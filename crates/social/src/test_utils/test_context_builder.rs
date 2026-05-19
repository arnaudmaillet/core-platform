// crates/social/src/test_utils/test_context_builder.rs

use crate::test_utils::SocialTestContext;
use fred::clients::Pool;
use scylla::client::session::Session;
use shared_kernel::test_utils::{TestContext, TestContextBuilder};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

pub struct SocialServerDeps {
    pub scylla: Arc<Session>,
    pub redis_repo: Arc<dyn shared_kernel::cache::CacheRepository>,
    pub redis_pool: Pool,
    pub kafka_brokers: Option<String>,
}

pub struct SocialTestContextBuilder<F = ()> {
    kernel_builder: TestContextBuilder<()>,
    server_factory: Option<F>,
    has_kafka: bool,
}

impl SocialTestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_scylla().with_redis(),
            server_factory: None,
            has_kafka: false,
        }
    }

    pub fn with_kafka(mut self) -> Self {
        self.kernel_builder = self.kernel_builder.with_kafka();
        self.has_kafka = true;
        self
    }

    pub fn with_server<F, Fut>(self, factory: F) -> SocialTestContextBuilder<F>
    where
        F: Fn(SocialServerDeps, SocketAddr, oneshot::Receiver<()>, oneshot::Sender<()>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        SocialTestContextBuilder {
            kernel_builder: self.kernel_builder,
            server_factory: Some(factory),
            has_kafka: self.has_kafka,
        }
    }
}

impl<F, Fut> SocialTestContextBuilder<F>
where
    F: Fn(SocialServerDeps, SocketAddr, oneshot::Receiver<()>, oneshot::Sender<()>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub async fn build_e2e(self) -> SocialTestContext {
        let kernel_infra = self.kernel_builder.build().await;

        // Extraction propre des ressources
        let redis_repo = kernel_infra.redis().repository();
        let redis_pool = kernel_infra.redis().repository().pool().clone();
        let scylla_session = kernel_infra.scylla().session();

        if let Some(factory) = self.server_factory {
            let deps = SocialServerDeps {
                scylla: scylla_session,
                redis_repo,
                redis_pool,
                kafka_brokers: if self.has_kafka {
                    Some(kernel_infra.kafka().bootstrap_servers().to_string())
                } else {
                    None
                },
            };

            let addr: SocketAddr = "[::1]:0".parse().unwrap();
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let actual_addr = listener.local_addr().unwrap();
            drop(listener);

            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let (ready_tx, ready_rx) = oneshot::channel();

            let server_handle = tokio::spawn(async move {
                factory(deps, actual_addr, shutdown_rx, ready_tx).await;
            });

            ready_rx.await.ok();

            let (pg, redis, scylla, kafka) = kernel_infra.into_parts();
            let final_kernel = TestContext::new(
                pg,
                redis,
                scylla,
                kafka,
                Some(actual_addr),
                Some(shutdown_tx),
                Some(server_handle),
            );
            return SocialTestContext::new(final_kernel);
        }

        SocialTestContext::new(kernel_infra)
    }
}
