use async_trait::async_trait;
use fred::interfaces::{KeysInterface, SetsInterface};
use redis_storage::RedisClient;
use uuid::Uuid;

use crate::application::port::FollowingStore;
use crate::domain::value_object::{AuthorId, ProfileId};
use crate::error::TimelineError;

fn following_key(profile_id: &ProfileId) -> String {
    format!("timeline:following:{}", profile_id)
}

fn fred_err(e: fred::error::Error) -> TimelineError {
    TimelineError::Redis(redis_storage::RedisStorageError::from(e))
}

/// Redis SET-backed following cache: `timeline:following:{profile_id}`.
///
/// Members are author UUIDs stored as hyphenated string representations.
/// No TTL — the SET is permanent until explicitly deleted or all members
/// are removed via SREM. Cold-start detection uses `EXISTS` (see `exists`).
pub struct RedisFollowingStore {
    client: RedisClient,
}

impl RedisFollowingStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl FollowingStore for RedisFollowingStore {
    async fn add(
        &self,
        follower_id: &ProfileId,
        followee_id: &AuthorId,
    ) -> Result<(), TimelineError> {
        self.client
            .inner
            .sadd::<i64, _, _>(following_key(follower_id), followee_id.to_string())
            .await
            .map_err(fred_err)?;
        Ok(())
    }

    async fn remove(
        &self,
        follower_id: &ProfileId,
        followee_id: &AuthorId,
    ) -> Result<(), TimelineError> {
        self.client
            .inner
            .srem::<i64, _, _>(following_key(follower_id), followee_id.to_string())
            .await
            .map_err(fred_err)?;
        Ok(())
    }

    async fn get_all(
        &self,
        follower_id: &ProfileId,
    ) -> Result<Vec<AuthorId>, TimelineError> {
        let members: Vec<String> = self
            .client
            .inner
            .smembers(following_key(follower_id))
            .await
            .map_err(fred_err)?;

        members
            .into_iter()
            .map(|s| {
                Uuid::parse_str(&s)
                    .map(AuthorId::from_uuid)
                    .map_err(|_| TimelineError::SocialGraphInvalidId(s))
            })
            .collect()
    }

    async fn exists(&self, follower_id: &ProfileId) -> Result<bool, TimelineError> {
        let exists: bool = self
            .client
            .inner
            .exists(following_key(follower_id))
            .await
            .map_err(fred_err)?;
        Ok(exists)
    }

    async fn set_all(
        &self,
        follower_id:  &ProfileId,
        followee_ids: &[AuthorId],
    ) -> Result<(), TimelineError> {
        if followee_ids.is_empty() {
            return Ok(());
        }
        let key     = following_key(follower_id);
        let members: Vec<String> = followee_ids.iter().map(|id| id.to_string()).collect();

        // DEL + SADD is not atomic but is safe: worst case the next SMEMBERS
        // during a race returns an empty set, triggering a rebuild on the next
        // request. The SET is rebuilt idempotently.
        let _: () = self.client.inner.del(key.clone()).await.map_err(fred_err)?;
        self.client
            .inner
            .sadd::<i64, _, _>(key, members)
            .await
            .map_err(fred_err)?;
        Ok(())
    }
}
