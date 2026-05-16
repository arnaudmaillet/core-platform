// crates/profile/src/test_utils/test_context_builder.rs

use crate::{test_utils::ProfileTestContext, utils::run_postgres_migrations};
use shared_kernel::test_utils::{E2EServerStarter, TestContextBuilder};

pub struct ProfileTestContextBuilder<S = ()> {
    kernel_builder: TestContextBuilder<S>,
}

impl ProfileTestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_postgres(&[]).with_redis(),
        }
    }
}

impl<S> ProfileTestContextBuilder<S> {
    pub fn with_server<NS: E2EServerStarter>(self, starter: NS) -> ProfileTestContextBuilder<NS> {
        ProfileTestContextBuilder {
            kernel_builder: self.kernel_builder.with_server(starter),
        }
    }

    pub fn with_kafka(mut self) -> Self {
        self.kernel_builder = self.kernel_builder.with_kafka();
        self
    }

    pub async fn build(self) -> ProfileTestContext {
        let kernel = self.kernel_builder.build().await;
        let ctx = ProfileTestContext::new(kernel);

        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply profile migrations");

        ctx
    }
}

/// Extension pour le mode E2E
impl<S: E2EServerStarter> ProfileTestContextBuilder<S> {
    pub async fn build_e2e(self) -> ProfileTestContext {
        let kernel = self.kernel_builder.build_e2e().await; // 💡 Conserve .build_e2e() pour les serveurs
        let ctx = ProfileTestContext::new(kernel);

        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply profile migrations");

        ctx
    }
}
