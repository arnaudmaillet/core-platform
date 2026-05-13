// crates/shared-kernel/src/test_utils/test_context_builder.rs

use crate::cache::CacheRepository;
use crate::test_utils::{PostgresTestContext, RedisTestContext, ScyllaTestContext, TestContext};
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::sync::oneshot;

#[async_trait]
pub trait E2EServerStarter: Send + Sync + 'static {
    async fn start_server(
        &self,
        pg_pool: sqlx::PgPool,
        redis_repo: std::sync::Arc<dyn CacheRepository>,
        addr: SocketAddr,
        shutdown_rx: oneshot::Receiver<()>,
    );
}

/// 3. Bloc spécifique pour le démarrage AVEC serveur (E2E)
impl<S: E2EServerStarter> TestContextBuilder<S> {
    pub async fn build_e2e(self) -> TestContext {
        let (pg, redis, scylla) = self.build_infrastructure().await;

        let starter = self.server_starter.expect("Server starter missing");

        // --- Setup Réseau ---
        let addr: SocketAddr = "[::1]:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind");
        let actual_addr = listener.local_addr().expect("Failed to get local addr");
        drop(listener);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // --- Préparation des dépendances pour le thread ---
        // On récupère les pools/repos seulement s'ils existent
        let pg_pool = pg
            .as_ref()
            .expect("E2E Server requires Postgres. Did you call .with_postgres()?")
            .pool()
            .clone();

        let redis_repo = redis
            .as_ref()
            .expect("E2E Server requires Redis. Did you call .with_redis()?")
            .repository();

        // --- Lancement du serveur ---
        let server_handle = tokio::spawn(async move {
            starter
                .start_server(pg_pool, redis_repo, actual_addr, shutdown_rx)
                .await;
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
    pg_migrations: Option<Vec<String>>,
    scylla_migrations: Option<Vec<String>>,
    with_redis: bool,
    server_starter: Option<S>,
}

impl TestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            pg_migrations: None,
            scylla_migrations: None,
            with_redis: false,
            server_starter: None,
        }
    }
}

impl<S> TestContextBuilder<S> {
    pub async fn build(self) -> TestContext {
        let (pg, redis, scylla) = self.build_infrastructure().await;
        TestContext::new(pg, redis, scylla, None, None, None)
    }

    pub fn with_postgres(mut self, migrations: &[&str]) -> Self {
        self.pg_migrations = Some(migrations.iter().map(|&s| s.to_string()).collect());
        self
    }

    pub fn with_redis(mut self) -> Self {
        self.with_redis = true;
        self
    }

    pub fn with_scylla(mut self, migrations: &[&str]) -> Self {
        self.scylla_migrations = Some(migrations.iter().map(|&s| s.to_string()).collect());
        self
    }

    pub fn with_server<NS: E2EServerStarter>(self, starter: NS) -> TestContextBuilder<NS> {
        TestContextBuilder {
            pg_migrations: self.pg_migrations,
            scylla_migrations: self.scylla_migrations,
            with_redis: self.with_redis,
            server_starter: Some(starter),
        }
    }

    /// La méthode build_infrastructure devient intelligente
    async fn build_infrastructure(
        &self,
    ) -> (
        Option<PostgresTestContext>,
        Option<RedisTestContext>,
        Option<ScyllaTestContext>,
    ) {
        let pg_future = async {
            if let Some(migs) = &self.pg_migrations {
                let migs_refs: Vec<&str> = migs.iter().map(|s| s.as_str()).collect();
                Some(
                    PostgresTestContext::builder()
                        .with_migrations(&migs_refs)
                        .build()
                        .await,
                )
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
            if let Some(migs) = &self.scylla_migrations {
                let migs_refs: Vec<&str> = migs.iter().map(|s| s.as_str()).collect();
                Some(
                    ScyllaTestContext::builder()
                        .with_migrations(&migs_refs)
                        .build()
                        .await,
                )
            } else {
                None
            }
        };

        // On lance uniquement ce qui est nécessaire en parallèle
        tokio::join!(pg_future, redis_future, scylla_future)
    }
}
