// crates/shared-kernel/src/infrastructure/postgres/utils/postgres_test_builder.rs

use crate::infrastructure::postgres::factories::PostgresConfig;
use crate::infrastructure::postgres::utils::PostgresTestContext;

pub struct PostgresTestContextBuilder {
    pub(crate) image_name: String,
    pub(crate) image_tag: String,
    pub(crate) user: String,
    pub(crate) password: String,
    pub(crate) db_name: String,
    pub(crate) migrations: Vec<String>,
    pub(crate) run_kernel_migrations: bool,
    pub(crate) config: Option<PostgresConfig>,
}

impl Default for PostgresTestContextBuilder {
    fn default() -> Self {
        Self {
            image_name: "postgis/postgis".to_string(),
            image_tag: "16-3.4-alpine".to_string(),
            user: "test".to_string(),
            password: "test".to_string(),
            db_name: "test_db".to_string(),
            migrations: Vec::new(),
            run_kernel_migrations: true,
            config: None,
        }
    }
}

impl PostgresTestContextBuilder {
    pub(crate)  fn new() -> Self {
        Self::default()
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

    pub fn with_config(mut self, config: PostgresConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build(self) -> PostgresTestContext {
        PostgresTestContext::restore(self).await
    }
}