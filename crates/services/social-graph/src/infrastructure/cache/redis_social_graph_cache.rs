use std::sync::Arc;

use async_trait::async_trait;
use fred::interfaces::{KeysInterface as _, SetsInterface as _};

use redis_storage::{RedisClient, RedisStorageError};

use crate::application::port::{RelationCounts, SocialGraphCache};
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

// ── Key builders ──────────────────────────────────────────────────────────────

fn following_key(profile_id: &ProfileId) -> String {
    format!("sg:following:v1:{profile_id}")
}

fn blocks_key(profile_id: &ProfileId) -> String {
    format!("sg:blocks:v1:{profile_id}")
}

fn followers_count_key(profile_id: &ProfileId) -> String {
    format!("sg:followers_count:v1:{profile_id}")
}

fn following_count_key(profile_id: &ProfileId) -> String {
    format!("sg:following_count:v1:{profile_id}")
}

// ── Error helper ──────────────────────────────────────────────────────────────

fn redis_err(e: fred::error::Error) -> SocialGraphError {
    SocialGraphError::Cache(RedisStorageError::from(e))
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct RedisSocialGraphCache {
    client: Arc<RedisClient>,
}

impl RedisSocialGraphCache {
    pub fn new(client: Arc<RedisClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SocialGraphCache for RedisSocialGraphCache {
    // ── Following set operations ──────────────────────────────────────────────

    async fn add_following(
        &self,
        follower_id: &ProfileId,
        followee_id: &ProfileId,
    ) -> Result<(), SocialGraphError> {
        let key    = following_key(follower_id);
        let member = followee_id.as_str();
        self.client
            .sadd::<i64, _, _>(&key, member)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn remove_following(
        &self,
        follower_id: &ProfileId,
        followee_id: &ProfileId,
    ) -> Result<(), SocialGraphError> {
        let key    = following_key(follower_id);
        let member = followee_id.as_str();
        self.client
            .srem::<i64, _, _>(&key, member)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    // ── Block set operations ──────────────────────────────────────────────────

    async fn add_block(
        &self,
        blocker_id: &ProfileId,
        blockee_id: &ProfileId,
    ) -> Result<(), SocialGraphError> {
        let key    = blocks_key(blocker_id);
        let member = blockee_id.as_str();
        self.client
            .sadd::<i64, _, _>(&key, member)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn remove_block(
        &self,
        blocker_id: &ProfileId,
        blockee_id: &ProfileId,
    ) -> Result<(), SocialGraphError> {
        let key    = blocks_key(blocker_id);
        let member = blockee_id.as_str();
        self.client
            .srem::<i64, _, _>(&key, member)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    // ── Counter operations ────────────────────────────────────────────────────

    async fn incr_followers_count(&self, profile_id: &ProfileId) -> Result<i64, SocialGraphError> {
        let key = followers_count_key(profile_id);
        let value: i64 = self.client.incr(&key).await.map_err(redis_err)?;
        Ok(value)
    }

    async fn decr_followers_count(&self, profile_id: &ProfileId) -> Result<i64, SocialGraphError> {
        let key   = followers_count_key(profile_id);
        let value: i64 = self.client.decr(&key).await.map_err(redis_err)?;
        // Clamp at zero: a decrement below zero signals a consistency gap.
        if value < 0 {
            let _ = self.client.set::<(), _, _>(&key, 0i64, None, None, false).await;
            return Ok(0);
        }
        Ok(value)
    }

    async fn incr_following_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError> {
        let key = following_count_key(profile_id);
        self.client.incr::<i64, _>(&key).await.map_err(redis_err)?;
        Ok(())
    }

    async fn decr_following_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError> {
        let key   = following_count_key(profile_id);
        let value: i64 = self.client.decr(&key).await.map_err(redis_err)?;
        if value < 0 {
            let _ = self.client.set::<(), _, _>(&key, 0i64, None, None, false).await;
        }
        Ok(())
    }

    async fn get_counts(&self, profile_id: &ProfileId) -> Result<RelationCounts, SocialGraphError> {
        let fk = followers_count_key(profile_id);
        let gk = following_count_key(profile_id);

        let (r_followers, r_following) = tokio::join!(
            self.client.get::<Option<i64>, _>(&fk),
            self.client.get::<Option<i64>, _>(&gk),
        );

        Ok(RelationCounts {
            followers: r_followers.map_err(redis_err)?.unwrap_or(0),
            following: r_following.map_err(redis_err)?.unwrap_or(0),
        })
    }
}
