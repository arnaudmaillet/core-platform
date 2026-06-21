use std::sync::Arc;

use async_trait::async_trait;
use fred::interfaces::KeysInterface as _;
use fred::types::Expiration;

use redis_storage::{RedisClient, RedisStorageError};

use crate::application::port::{ProfileCache, ProfileView};
use crate::domain::value_object::{AccountId, ProfileId};
use crate::error::ProfileError;

const PROFILE_TTL_SECS: i64 = 300;
const HANDLE_TTL_SECS:  i64 = 600;

fn redis_err(e: fred::error::Error) -> ProfileError {
    ProfileError::Cache(RedisStorageError::from(e))
}

fn profile_key(id: &ProfileId) -> String {
    format!("profile:v1:{}", id.as_str())
}

fn handle_key(handle: &str) -> String {
    format!("handle:v1:{handle}")
}

fn account_profiles_key(account_id: &AccountId) -> String {
    format!("account:profiles:v1:{}", account_id.as_str())
}

pub struct RedisProfileCache {
    client: Arc<RedisClient>,
}

impl RedisProfileCache {
    pub fn new(client: Arc<RedisClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ProfileCache for RedisProfileCache {
    async fn get_by_id(&self, id: &ProfileId) -> Result<Option<ProfileView>, ProfileError> {
        let key = profile_key(id);
        let raw: Option<String> = self.client
            .get(&key)
            .await
            .map_err(redis_err)?;

        raw.map(|s| {
            serde_json::from_str::<ProfileView>(&s).map_err(|e| ProfileError::DomainViolation {
                field: "cache:profile_view".to_string(),
                message: e.to_string(),
            })
        })
        .transpose()
    }

    async fn set_by_id(&self, view: &ProfileView) -> Result<(), ProfileError> {
        let key = profile_key(&ProfileId::try_from(view.id.as_str())?);
        let json = serde_json::to_string(view).map_err(|e| ProfileError::DomainViolation {
            field: "cache:serialize".to_string(),
            message: e.to_string(),
        })?;
        self.client
            .set::<(), _, _>(&key, json, Some(Expiration::EX(PROFILE_TTL_SECS)), None, false)
            .await
            .map_err(redis_err)
    }

    async fn invalidate_by_id(&self, id: &ProfileId) -> Result<(), ProfileError> {
        let key = profile_key(id);
        self.client.del::<(), _>(&key).await.map_err(redis_err)
    }

    async fn get_profile_id_by_handle(
        &self,
        handle: &str,
    ) -> Result<Option<ProfileId>, ProfileError> {
        let key = handle_key(handle);
        let raw: Option<String> = self.client.get(&key).await.map_err(redis_err)?;
        raw.map(|s| ProfileId::try_from(s.as_str()))
            .transpose()
    }

    async fn set_handle_mapping(
        &self,
        handle: &str,
        id: ProfileId,
    ) -> Result<(), ProfileError> {
        let key = handle_key(handle);
        self.client
            .set::<(), _, _>(&key, id.as_str(), Some(Expiration::EX(HANDLE_TTL_SECS)), None, false)
            .await
            .map_err(redis_err)
    }

    async fn invalidate_handle(&self, handle: &str) -> Result<(), ProfileError> {
        let key = handle_key(handle);
        self.client.del::<(), _>(&key).await.map_err(redis_err)
    }

    async fn invalidate_account_profiles(
        &self,
        account_id: &AccountId,
    ) -> Result<(), ProfileError> {
        let key = account_profiles_key(account_id);
        self.client.del::<(), _>(&key).await.map_err(redis_err)
    }
}
