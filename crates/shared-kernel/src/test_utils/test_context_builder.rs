// crates/shared-kernel/src/test_utils/test_context_builder.rs

use crate::test_utils::PostgresTestContextBuilder;
use crate::test_utils::{
    KafkaTestContext, PostgresTestContext, RedisTestContext, ScyllaTestContext, TestContext,
};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

#[async_trait]
pub trait E2EServerStarter: Send + Sync + 'static {
    async fn start_server(
        &self,
        addr: SocketAddr,
        shutdown_rx: oneshot::Receiver<()>,
        ctx: &TestContext,
    );
}
pub struct TestContextBuilder<S = ()> {
    postgres_builder: Option<PostgresTestContextBuilder>,
    with_scylla: bool,
    with_redis: bool,
    with_kafka: bool,
    server_starter: Option<S>,
}

impl TestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            postgres_builder: None,
            with_scylla: false,
            with_redis: false,
            with_kafka: false,
            server_starter: None,
        }
    }
}

impl<S> TestContextBuilder<S> {
    pub async fn build(self) -> TestContext {
        let (pg, redis, scylla, kafka) = self.build_infrastructure().await;
        TestContext::new(pg, redis, scylla, kafka, None, None, None)
    }

    pub fn with_postgres<P: AsRef<std::path::Path>, I: IntoIterator<Item = P>>(
        mut self,
        migration_paths: I,
    ) -> Self {
        let pg_builder = self
            .postgres_builder
            .get_or_insert_with(PostgresTestContextBuilder::default);

        for p in migration_paths {
            let path_str = p.as_ref().to_string_lossy().into_owned();
            if !pg_builder.migrations.contains(&path_str) {
                pg_builder.migrations.push(path_str);
            }
        }

        // 3. On active explicitement les migrations kernel
        pg_builder.run_kernel_migrations = true;

        self
    }

    pub fn with_redis(mut self) -> Self {
        self.with_redis = true;
        self
    }

    pub fn with_scylla(mut self) -> Self {
        self.with_scylla = true;
        self
    }

    pub fn with_kafka(mut self) -> Self {
        self.with_kafka = true;
        self
    }

    pub fn with_server<NS: E2EServerStarter>(self, starter: NS) -> TestContextBuilder<NS> {
        TestContextBuilder {
            postgres_builder: self.postgres_builder,
            with_scylla: self.with_scylla,
            with_redis: self.with_redis,
            with_kafka: self.with_kafka,
            server_starter: Some(starter),
        }
    }

    async fn build_infrastructure(
        self,
    ) -> (
        Option<PostgresTestContext>,
        Option<RedisTestContext>,
        Option<ScyllaTestContext>,
        Option<KafkaTestContext>,
    ) {
        tracing::info!("🚀 Starting infrastructure services...");
        let pg_future = async {
            if let Some(builder) = self.postgres_builder {
                Some(builder.build().await)
            } else {
                None
            }
        };

        let redis_future = async {
            if self.with_redis {
                Some(RedisTestContext::builder().build().await)
            } else {
                None
            }
        };

        let scylla_future = async {
            if self.with_scylla {
                Some(ScyllaTestContext::builder().build().await)
            } else {
                None
            }
        };

        let kafka_future = async {
            if self.with_kafka {
                Some(KafkaTestContext::builder().build().await)
            } else {
                None
            }
        };

        let (pg, redis, scylla, kafka) =
            tokio::join!(pg_future, redis_future, scylla_future, kafka_future);

        tracing::info!("✅ Infrastructure services ready");
        (pg, redis, scylla, kafka)
    }
}

/// 3. Bloc spécifique pour le démarrage AVEC serveur (E2E)
impl<S: E2EServerStarter> TestContextBuilder<S> {
    pub async fn build_e2e(mut self) -> TestContext {
        tracing::info!("🛠 Starting E2E test environment...");

        // 1. On extrait uniquement le starter, on laisse le reste dans self
        let starter = self.server_starter.take().expect("Server starter missing");

        // 2. On build l'infra en utilisant self (qui contient postgres_builder, etc.)
        let (pg, redis, scylla, kafka) = self.build_infrastructure().await;

        // 3. On crée le contexte maintenant
        let ctx = TestContext::new(pg, redis, scylla, kafka, None, None, None);

        // 4. Setup Réseau
        let addr: SocketAddr = "[::1]:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind");
        let actual_addr = listener.local_addr().expect("Failed to get local addr");
        drop(listener);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // 5. Pour le spawn, on utilise un Arc pour le contexte afin qu'il soit 'static
        let ctx_shared = std::sync::Arc::new(ctx);
        let ctx_for_server = ctx_shared.clone();

        tracing::info!(server_addr = %actual_addr, "⚙️ Launching gRPC server...");
        let server_handle = tokio::spawn(async move {
            // On passe la référence du contexte au starter
            starter
                .start_server(actual_addr, shutdown_rx, &ctx_for_server)
                .await;
        });

        if !Self::wait_for_server(actual_addr, 30000).await {
            panic!("❌ E2E Server failed to start");
        }

        // 6. On reconstruit un TestContext final avec les infos serveur
        // On doit extraire les options du ctx_shared
        let (pg, redis, scylla, kafka) = Arc::try_unwrap(ctx_shared).unwrap().into_parts();

        tracing::info!("🏁 E2E environment fully operational");
        TestContext::new(
            pg,
            redis,
            scylla,
            kafka,
            Some(actual_addr),
            Some(shutdown_tx),
            Some(server_handle),
        )
    }

    async fn wait_for_server(addr: SocketAddr, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let mut attempts = 0;

        println!("⏳ Waiting for gRPC server to boot on {}...", addr);

        while start.elapsed().as_millis() < timeout_ms as u128 {
            attempts += 1;

            match tokio::net::TcpStream::connect(addr).await {
                Ok(_) => {
                    println!(
                        "✅ Server ready after {}ms ({} attempts)",
                        start.elapsed().as_millis(),
                        attempts
                    );
                    return true;
                }
                Err(_) => {
                    if attempts % 100 == 0 {
                        println!(
                            "   ... still waiting (elapsed: {}s)",
                            start.elapsed().as_secs()
                        );
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
        false
    }
}
