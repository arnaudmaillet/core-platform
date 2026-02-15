// crates/shared-kernel/src/infrastructure/utils/infrastructure_kernel_test_context_builder.rs

use crate::infrastructure::postgres::utils::PostgresTestContext;
use crate::infrastructure::redis::utils::RedisTestContext;
use crate::infrastructure::scylla::utils::ScyllaTestContext;
use crate::infrastructure::utils::InfrastructureKernelTestContext;


pub struct InfrastructureKernelTestBuilder {
    pg_migrations: Vec<String>,
    scylla_migrations: Vec<String>,
}

impl InfrastructureKernelTestBuilder {
    pub fn new() -> Self {
        Self {
            pg_migrations: Vec::new(),
            scylla_migrations: Vec::new(),
        }
    }

    pub fn with_postgres_migrations(mut self, paths: &[&str]) -> Self {
        self.pg_migrations = paths.iter().map(|&s| s.to_string()).collect();
        self
    }

    pub fn with_scylla_migrations(mut self, paths: &[&str]) -> Self {
        self.scylla_migrations = paths.iter().map(|&s| s.to_string()).collect();
        self
    }

    pub async fn build(self) -> InfrastructureKernelTestContext {
        let pg_migs: Vec<&str> = self.pg_migrations.iter().map(|s| s.as_str()).collect();
        let scylla_migs: Vec<&str> = self.scylla_migrations.iter().map(|s| s.as_str()).collect();

        // Parallélisation du démarrage des containers de test
        let (postgres_ctx, redis_ctx, scylla_ctx) = tokio::join!(
            PostgresTestContext::builder().with_migrations(&pg_migs).build(),
            RedisTestContext::builder().build(),
            ScyllaTestContext::builder().with_migrations(&scylla_migs).build()
        );

        InfrastructureKernelTestContext::new(
            postgres_ctx,
            redis_ctx,
            scylla_ctx,
        )
    }
}