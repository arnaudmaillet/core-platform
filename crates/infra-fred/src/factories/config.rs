// crates/shared-kernel/src/infrastructure/redis/factories/redis_config.rs

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub max_clients: usize,
}