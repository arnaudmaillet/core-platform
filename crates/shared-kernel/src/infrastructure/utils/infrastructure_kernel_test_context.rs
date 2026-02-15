// crates/shared-kernel/src/infrastructure/utils/infrastructure_test_context.rs

#![cfg(feature = "test-utils")]

use crate::infrastructure::postgres::utils::PostgresTestContext;
use crate::infrastructure::redis::utils::RedisTestContext;
use crate::infrastructure::scylla::utils::ScyllaTestContext;
use crate::infrastructure::utils::InfrastructureKernelTestBuilder;

/// Le contexte final encapsulant toutes les ressources de test.
/// Les champs sont privés pour garantir l'utilisation des getters.
pub struct InfrastructureKernelTestContext {
    postgres_ctx: PostgresTestContext,
    redis_ctx: RedisTestContext,
    scylla_ctx: ScyllaTestContext,
}

impl InfrastructureKernelTestContext {
    pub(crate) fn new(
        postgres_ctx: PostgresTestContext,
        redis_ctx: RedisTestContext,
        scylla_ctx: ScyllaTestContext,
    ) -> Self {
        Self {
            postgres_ctx,
            redis_ctx,
            scylla_ctx,
        }
    }

    // --- Getters pour accéder aux Contextes de Test ---

    pub fn postgres(&self) -> &PostgresTestContext {
        &self.postgres_ctx
    }

    pub fn redis(&self) -> &RedisTestContext {
        &self.redis_ctx
    }

    pub fn scylla(&self) -> &ScyllaTestContext {
        &self.scylla_ctx
    }

    pub fn builder() -> InfrastructureKernelTestBuilder {
        InfrastructureKernelTestBuilder::new()
    }
}