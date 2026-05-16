// crates/profile/src/test_utils/test_context.rs

use crate::test_utils::ProfileTestContextBuilder;
use shared_kernel::test_utils::TestContext;
use sqlx::PgPool;

pub struct ProfileTestContext {
    kernel_context: TestContext,
}

impl ProfileTestContext {
    pub fn builder() -> ProfileTestContextBuilder {
        ProfileTestContextBuilder::new()
    }

    /// Getter pour accéder aux ressources du noyau (Postgres, Scylla, Redis)
    pub fn kernel(&self) -> &TestContext {
        &self.kernel_context
    }

    pub fn pg_pool(&self) -> PgPool {
        self.kernel_context.postgres().pool().clone()
    }

    pub async fn shutdown(self) {
        self.kernel_context.shutdown().await;
    }

    /// Constructeur interne utilisé par le builder
    pub(crate) fn new(kernel_context: TestContext) -> Self {
        Self { kernel_context }
    }
}
