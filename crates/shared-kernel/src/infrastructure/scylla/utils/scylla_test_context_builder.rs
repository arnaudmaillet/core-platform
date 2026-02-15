// crates/shared-kernel/src/infrastructure/scylla/utils/scylla_test_builder.rs

use crate::infrastructure::scylla::factories::ScyllaConfig;
use crate::infrastructure::scylla::utils::scylla_test_context::ScyllaTestContext;

pub struct ScyllaTestContextBuilder {
    pub(crate) image_name: String,
    pub(crate) image_tag: String,
    pub(crate) keyspace: String,
    pub(crate) migrations: Vec<String>,
    pub(crate) run_kernel_migrations: bool,
    pub(crate) config: Option<ScyllaConfig>,
}

impl Default for ScyllaTestContextBuilder {
    fn default() -> Self {
        Self {
            image_name: "scylladb/scylla".to_string(),
            image_tag: "6.2.1".to_string(),
            keyspace: "it".to_string(), // must be short (max 16 char)
            migrations: Vec::new(),
            run_kernel_migrations: true,
            config: None,
        }
    }
}

impl ScyllaTestContextBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn with_keyspace(mut self, ks: &str) -> Self {
        self.keyspace = ks.to_string();
        self
    }

    pub fn with_migrations(mut self, paths: &[&str]) -> Self {
        self.migrations = paths.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn skip_kernel_migrations(mut self) -> Self {
        self.run_kernel_migrations = false;
        self
    }

    pub fn with_image(mut self, name: &str, tag: &str) -> Self {
        self.image_name = name.to_string();
        self.image_tag = tag.to_string();
        self
    }

    pub fn with_config(mut self, config: ScyllaConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build(self) -> ScyllaTestContext {
        ScyllaTestContext::restore(self).await
    }
}