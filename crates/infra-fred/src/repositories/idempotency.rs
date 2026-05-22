// crates/shared-kernel/src/persistence/redis/repositories/idempotency.rs

use async_trait::async_trait;
use fred::clients::Pool;
use fred::interfaces::KeysInterface;
use fred::types::{Expiration, SetOptions};
use shared_kernel::core::{Error, Result, Transaction};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

pub struct RedisIdempotencyRepository {
    pool: Pool,
    namespace: String,
    ttl_seconds: u64,
}

impl RedisIdempotencyRepository {
    pub fn new(pool: Pool, namespace: impl Into<String>, ttl_seconds: u64) -> Self {
        Self {
            pool,
            namespace: namespace.into(),
            ttl_seconds,
        }
    }

    fn make_key(&self, command_id: &Uuid) -> String {
        format!("idempotency:{}:{}", self.namespace, command_id)
    }
}

#[async_trait]
impl IdempotencyRepository for RedisIdempotencyRepository {
    async fn exists(
        &self,
        _tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<bool> {
        let key = self.make_key(command_id);
        let count: i64 = self
            .pool
            .exists(key)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
        Ok(count > 0)
    }

    async fn save(
        &self,
        _tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<()> {
        let key = self.make_key(command_id);

        let expiration = Some(Expiration::EX(self.ttl_seconds as i64));
        let set_options = Some(SetOptions::NX);

        let result: Option<String> = self
            .pool
            .set(&key, "processed", expiration, set_options, false)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        if result.is_none() {
            return Err(Error::already_exists(
                "Command",
                "id",
                command_id.to_string(),
            ));
        }

        Ok(())
    }
}
