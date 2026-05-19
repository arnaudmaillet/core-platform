// crates/social/src/test_utils/test_context.rs

use crate::test_utils::SocialTestContextBuilder;
use shared_kernel::test_utils::TestContext;

pub struct SocialTestContext {
    kernel: TestContext,
}

impl SocialTestContext {
    pub fn new(kernel: TestContext) -> Self {
        Self { kernel }
    }

    pub fn builder() -> SocialTestContextBuilder {
        SocialTestContextBuilder::new()
    }

    pub fn kernel(&self) -> &TestContext {
        &self.kernel
    }

    pub async fn shutdown(self) {
        drop(self.kernel);
    }
}
