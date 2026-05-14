// crates/account/src/test_utils/test_context_builder.rs

use crate::{test_utils::AccountTestContext, utils::run_postgres_migrations};
use shared_kernel::test_utils::{E2EServerStarter, TestContextBuilder};

pub struct AccountTestContextBuilder<S = ()> {
    kernel_builder: TestContextBuilder<S>,
}

impl AccountTestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_postgres(&[]).with_redis(),
        }
    }
}

impl<S> AccountTestContextBuilder<S> {
    pub fn with_server<NS: E2EServerStarter>(self, starter: NS) -> AccountTestContextBuilder<NS> {
        AccountTestContextBuilder {
            kernel_builder: self.kernel_builder.with_server(starter),
        }
    }

    /// Build classique (Unit/Integration)
    pub async fn build(self) -> AccountTestContext {
        let kernel = self.kernel_builder.build().await;
        let ctx = AccountTestContext::new(kernel);

        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply account migrations");

        ctx
    }
}

/// Extension pour le mode E2E
impl<S: E2EServerStarter> AccountTestContextBuilder<S> {
    pub async fn build_e2e(self) -> AccountTestContext {
        let kernel = self.kernel_builder.build_e2e().await;
        let ctx = AccountTestContext::new(kernel);

        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply account migrations");

        ctx
    }
}
