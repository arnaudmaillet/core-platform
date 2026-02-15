// crates/shared-kernel/src/infrastructure/redis/utils/redis_test_context.rs

use std::sync::Arc;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis as RedisImage;
use crate::infrastructure::redis::factories::RedisContext;
use crate::infrastructure::redis::repositories::RedisCacheRepository;
use crate::infrastructure::redis::utils::redis_test_builder::RedisTestContextBuilder;

pub struct RedisTestContext {
    context: RedisContext,
    pub container: ContainerAsync<RedisImage>,
}

impl RedisTestContext {
    pub fn builder() -> RedisTestContextBuilder {
        RedisTestContextBuilder::new()
    }

    pub fn repository(&self) -> Arc<RedisCacheRepository> {
        self.context.repository()
    }

    pub fn url(&self) -> String {
        self.context.url()
    }

    pub(crate) async fn restore(builder: RedisTestContextBuilder) -> Self {
        // 1. Démarrage de l'infrastructure physique (Docker)
        let container = RedisImage::default()
            .with_tag(&builder.image_tag)
            .start()
            .await
            .expect("Échec du démarrage de Redis");

        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(builder.container_port).await.unwrap();
        let url = format!("redis://{}:{}", host, port);

        // 2. Création du contexte logique (Production)
        // On utilise builder_raw() pour injecter l'URL du container sans lire l'ENV
        let mut redis_builder = RedisContext::builder_raw()
            .with_url(&url);

        // On injecte la config si le test en a défini une
        if let Some(cfg) = builder.config {
            redis_builder = redis_builder.with_max_clients(cfg.max_clients);
        }

        let context = redis_builder.build().await
            .expect("Failed to build RedisContext for tests");

        Self { context, container }
    }
}