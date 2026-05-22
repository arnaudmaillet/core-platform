// crates/shared-kernel/src/test_utils/kafka_test_builder.rs

use crate::KafkaTestContext;
use std::collections::HashMap;

pub struct KafkaTestContextBuilder {
    pub(crate) image_name: String,
    pub(crate) image_tag: String,
    pub(crate) config: Option<HashMap<String, String>>,
}

impl Default for KafkaTestContextBuilder {
    fn default() -> Self {
        Self {
            image_name: "confluentinc/cp-kafka".to_string(),
            image_tag: "7.4.0".to_string(),
            config: None,
        }
    }
}

impl KafkaTestContextBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_image(mut self, name: &str, tag: &str) -> Self {
        self.image_name = name.to_string();
        self.image_tag = tag.to_string();
        self
    }

    pub fn with_rdkafka_property(mut self, key: &str, value: &str) -> Self {
        let mut cfg = self.config.unwrap_or_default();
        cfg.insert(key.to_string(), value.to_string());
        self.config = Some(cfg);
        self
    }

    pub async fn build(self) -> KafkaTestContext {
        KafkaTestContext::restore(self).await
    }
}
