// crates/social/src/application/builder.rs

use infra_fred::fred::clients::Pool as RedisPool;
use infra_scylla::scylla::client::session::Session as ScyllaSession;
use std::sync::Arc;

use crate::{
    commands::{FollowCommand, FollowHandler, UnfollowCommand, UnfollowHandler},
    context::{SocialAppContext, SocialContext},
    domain::repositories::{CounterRepository, RelationRepository},
    redis::RedisCounterRepository,
    scylla::{ScyllaCounterRepository, ScyllaRelationRepository},
};

// Imports du Shared Kernel
use shared_kernel::{
    cache::CacheRepository, command::CommandBus, idempotency::IdempotencyRepository,
};

pub struct SocialServiceBuilder {
    scylla_session: Arc<ScyllaSession>,
    redis_pool: RedisPool,
    redis_cache_repo: Arc<dyn CacheRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl SocialServiceBuilder {
    pub fn new(
        scylla_session: Arc<ScyllaSession>,
        redis_pool: RedisPool,
        redis_cache_repo: Arc<dyn CacheRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            scylla_session,
            redis_pool,
            redis_cache_repo,
            idempotency_repo,
        }
    }

    /// Construit le contexte global de l'application Social (Clean Architecture)
    pub async fn build_context(&self) -> Arc<SocialAppContext> {
        // 1. Initialisation des dépôts d'infrastructure ScyllaDB (Persistance)
        let relation_repo: Arc<dyn RelationRepository> = Arc::new(
            ScyllaRelationRepository::new(self.scylla_session.clone())
                .await
                .expect("💥 Impossible d'initialiser ScyllaRelationRepository"),
        );

        let scylla_counter_repo: Arc<dyn CounterRepository> = Arc::new(
            ScyllaCounterRepository::new(self.scylla_session.clone())
                .await
                .expect("💥 Impossible d'initialiser ScyllaCounterRepository"),
        );

        // 2. Initialisation du dépôt de cache Redis pour les compteurs à chaud
        let redis_counter_repo: Arc<dyn CounterRepository> =
            Arc::new(RedisCounterRepository::new(self.redis_pool.clone()));

        // 3. Instanciation finale de ton SocialAppContext typé
        Arc::new(SocialAppContext::new(
            relation_repo,
            redis_counter_repo,
            scylla_counter_repo,
            self.idempotency_repo.clone(),
        ))
    }

    /// Enregistre tous les handlers d'écriture graph/compteurs dans le CommandBus
    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.redis_cache_repo.clone());

        bus.register::<SocialContext, FollowCommand, FollowHandler>(FollowHandler);

        bus.register::<SocialContext, UnfollowCommand, UnfollowHandler>(UnfollowHandler);

        Arc::new(bus)
    }
}
