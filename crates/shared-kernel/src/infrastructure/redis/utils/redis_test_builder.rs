// crates/shared-kernel/src/infrastructure/redis/utils/redis_test_builder.rs

use crate::infrastructure::redis::factories::RedisConfig;
use crate::infrastructure::redis::utils::redis_test_context::RedisTestContext;

pub struct RedisTestContextBuilder {
    pub(crate) image_tag: String,
    pub(crate) container_port: u16,
    pub(crate) config: Option<RedisConfig>,
}

impl Default for RedisTestContextBuilder {
    fn default() -> Self {
        Self {
            image_tag: "7.2-alpine".to_string(),
            container_port: 6379,
            config: None,
        }
    }
}

impl RedisTestContextBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_image(mut self, tag: &str) -> Self {
        self.image_tag = tag.to_string();
        self
    }

    pub fn with_config(mut self, config: RedisConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build(self) -> RedisTestContext {
        RedisTestContext::restore(self).await
    }
}