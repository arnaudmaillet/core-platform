// crates/shared-kernel/src/test_utils/test_context_builder.rs

use crate::test_utils::{
    KafkaTestContext, PostgresTestContext, RedisTestContext, ScyllaTestContext, TestContext,
};
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::sync::oneshot;

#[async_trait]
pub trait E2EServerStarter: Send + Sync + 'static {
    async fn start_server(&self, addr: SocketAddr, shutdown_rx: oneshot::Receiver<()>);
}

/// 3. Bloc spécifique pour le démarrage AVEC serveur (E2E)
impl<S: E2EServerStarter> TestContextBuilder<S> {
    pub async fn build_e2e(self) -> TestContext {
        let (pg, redis, scylla, kafka) = self.build_infrastructure().await;

        let starter = self.server_starter.expect("Server starter missing");

        // --- Setup Réseau ---
        let addr: SocketAddr = "[::1]:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind");
        let actual_addr = listener.local_addr().expect("Failed to get local addr");
        drop(listener);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // --- Lancement du serveur (Plus aucun argument lié à la DB !) ---
        let server_handle = tokio::spawn(async move {
            starter.start_server(actual_addr, shutdown_rx).await;
        });

        if !Self::wait_for_server(actual_addr, 30000).await {
            panic!(
                "❌ E2E Server failed to start on {} within 30s. \
              Check if Keycloak is reachable or if the server crashed.",
                actual_addr
            );
        }

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

pub struct TestContextBuilder<S = ()> {
    with_postgres: bool,
    with_scylla: bool,
    with_redis: bool,
    with_kafka: bool,
    server_starter: Option<S>,
}

impl TestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            with_postgres: false,
            with_scylla: false,
            with_redis: false,
            with_kafka: false,
            server_starter: None,
        }
    }
}
impl<S> TestContextBuilder<S> {
    pub async fn build(self) -> TestContext {
        let (pg, redis, scylla, kafka) = self.build_infrastructure().await; // AJOUT : kafka
        TestContext::new(pg, redis, scylla, kafka, None, None, None) // AJOUT : kafka
    }

    pub fn with_postgres(mut self) -> Self {
        self.with_postgres = true;
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
            with_postgres: self.with_postgres,
            with_scylla: self.with_scylla,
            with_redis: self.with_redis,
            with_kafka: self.with_kafka,
            server_starter: Some(starter),
        }
    }

    async fn build_infrastructure(
        &self,
    ) -> (
        Option<PostgresTestContext>,
        Option<RedisTestContext>,
        Option<ScyllaTestContext>,
        Option<KafkaTestContext>,
    ) {
        let pg_future = async {
            if self.with_postgres {
                Some(PostgresTestContext::builder().build().await)
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
                // Initialisation brute sans arguments de fichiers
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

        (pg, redis, scylla, kafka)
    }
}
