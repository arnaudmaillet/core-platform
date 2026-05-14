// crates/account/src/test_utils/test_context.rs

use crate::test_utils::AccountTestContextBuilder;
use shared_kernel::test_utils::TestContext;
use sqlx::PgPool;

pub struct AccountTestContext {
    kernel_context: TestContext,
}

impl AccountTestContext {
    pub fn builder() -> AccountTestContextBuilder {
        AccountTestContextBuilder::new()
    }

    /// Raccourci pour le cas le plus courant (setup standard du profil)
    pub async fn setup() -> Self {
        Self::builder().build().await
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
