// crates/account/src/infrastructure/factories/context_factory.rs

use std::sync::Arc;
use crate::application::context::AccountContext;
use crate::infrastructure::postgres::repositories::{
    PostgresAccountIdentityRepository,
    PostgresAccountMetadataRepository,
    PostgresAccountSettingsRepository
};
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::sharding::ShardResolver;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

pub struct AccountContextFactory {
    resolver: Arc<ShardResolver>,
}

impl AccountContextFactory {
    pub fn new(resolver: Arc<ShardResolver>) -> Self {
        Self { resolver }
    }

    pub fn create(&self, account_id: AccountId, region: RegionCode) -> AccountContext {
        // 1. Résolution du Shard (Géo + Modulo)
        let node = self.resolver.resolve(&account_id, &region)
            .expect("CRITICAL: Failed to resolve shard. Infrastructure inconsistent.");

        let storage = &node.storage;
        let pool = storage.postgres.clone().expect("Shard missing Postgres pool");
        let cache = storage.redis.clone();

        // 2. Instanciation des repositories pour ce shard spécifique
        let identity_repo = Arc::new(PostgresAccountIdentityRepository::new(pool.clone(), cache.clone()));
        let metadata_repo = Arc::new(PostgresAccountMetadataRepository::new(pool.clone(), cache.clone()));
        let settings_repo = Arc::new(PostgresAccountSettingsRepository::new(pool.clone(), cache.clone()));
        let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));

        // 3. Assemblage du contexte
        AccountContext::new(
            account_id,
            region,
            identity_repo,
            metadata_repo,
            settings_repo,
            outbox_repo, 
            pool,
        )
    }
}