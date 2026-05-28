// crates/social/src/infrastructure/redis/repositories/counter.rs

use async_trait::async_trait;
use chrono::Utc;
use infra_fred::fred::clients::Pool;
use infra_fred::fred::interfaces::{HashesInterface, SetsInterface};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;
use std::collections::HashMap;

use crate::domain::entities::ProfileCounters;
use crate::domain::repositories::CounterRepository;

pub struct RedisCounterRepository {
    pool: Pool,
}

impl RedisCounterRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    fn make_key(&self, profile_id: &ProfileId) -> String {
        format!("profile:counters:{}", profile_id)
    }
}

#[async_trait]
impl CounterRepository for RedisCounterRepository {
    async fn increment_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let follower_key = self.make_key(&follower_id);
        let following_key = self.make_key(&following_id);

        self.pool
            .hincrby::<i64, _, _>(&follower_key, "following", 1)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        self.pool
            .hincrby::<i64, _, _>(&following_key, "followers", 1)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        self.pool
            .sadd::<i64, _, _>(
                "profiles:dirty",
                vec![follower_id.to_string(), following_id.to_string()],
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        Ok(())
    }

    async fn decrement_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let follower_key = self.make_key(&follower_id);
        let following_key = self.make_key(&following_id);

        self.pool
            .hincrby::<i64, _, _>(&follower_key, "following", -1)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        self.pool
            .hincrby::<i64, _, _>(&following_key, "followers", -1)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        self.pool
            .sadd::<i64, _, _>(
                "profiles:dirty",
                vec![follower_id.to_string(), following_id.to_string()],
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        Ok(())
    }

    async fn get_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        let key = self.make_key(&profile_id);

        let raw_values: HashMap<String, i64> = self
            .pool
            .hgetall(&key)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        if raw_values.is_empty() {
            return Err(Error::not_found("ProfileCounters", profile_id.to_string()));
        }

        let followers_raw = raw_values.get("followers").cloned().unwrap_or(0);
        let following_raw = raw_values.get("following").cloned().unwrap_or(0);

        Ok(ProfileCounters::restore(
            profile_id,
            followers_raw.try_into()?,
            following_raw.try_into()?,
            1,
            Utc::now(),
            Utc::now(),
        ))
    }

    async fn save(&self, counters: &ProfileCounters) -> Result<()> {
        let key = self.make_key(counters.profile_id());

        let mut map = HashMap::new();
        map.insert("followers", counters.followers_count().to_string());
        map.insert("following", counters.following_count().to_string());

        self.pool
            .hset::<(), _, _>(&key, map)
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        Ok(())
    }
}
