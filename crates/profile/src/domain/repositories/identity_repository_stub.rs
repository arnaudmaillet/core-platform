// crates/profile/src/utils/test_utils.rs
#![cfg(test)]

use futures::Future;
use std::any::Any;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::domain::entities::Profile;
use crate::domain::repositories::{
    ProfileIdentityRepository, ProfileRepository, ProfileStatsRepository,
};
use crate::domain::value_objects::{Handle, ProfileId, ProfileStats};
use shared_kernel::domain::events::{DomainEvent, EventEnvelope};
use shared_kernel::domain::repositories::{CacheRepository, OutboxRepository};
use shared_kernel::domain::transaction::{Transaction, TransactionManager};
use shared_kernel::domain::value_objects::{AccountId, Counter, RegionCode, Username};
use shared_kernel::errors::{AppError, AppResult, ErrorCode, Result};

// --- STUB PROFILE REPOSITORY (Postgres) ---
pub struct ProfileRepositoryStub {
    pub profile_to_return: Mutex<Option<Profile>>,
    pub exists_return: Mutex<bool>,
    pub error_to_return: Mutex<Option<shared_kernel::errors::DomainError>>,
}

impl Default for ProfileRepositoryStub {
    fn default() -> Self {
        Self {
            profile_to_return: Mutex::new(None),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl ProfileIdentityRepository for ProfileRepositoryStub {
    async fn save(&self, _p: &Profile, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
    async fn fetch(&self, _id: &ProfileId, _r: &RegionCode) -> Result<Option<Profile>> {
        Ok(self.profile_to_return.lock().unwrap().clone())
    }
    async fn fetch_by_handle(&self, _: &Handle, _: &RegionCode) -> Result<Option<Profile>> {
        Ok(self.profile_to_return.lock().unwrap().clone())
    }

    async fn fetch_all_by_owner(&self, _: &AccountId) -> Result<Vec<Profile>> {
        Ok(self.profile_to_return.lock().unwrap().clone().into_iter().collect())
    }

    async fn exists_by_handle(&self, _h: &Handle, _r: &RegionCode) -> Result<bool> {
        Ok(*self.exists_return.lock().unwrap())
    }
    async fn delete(&self, _: &ProfileId, _: &RegionCode) -> Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ProfileRepository for ProfileRepositoryStub {
    async fn assemble_full_profile(&self, id: &ProfileId, r: &RegionCode, ) -> Result<Option<Profile>> {
        self.fetch(id, r).await
    }
    async fn resolve_profile_from_handle(&self, h: &Handle, r: &RegionCode, ) -> Result<Option<Profile>> {
        self.fetch_by_handle(h, r).await
    }
    async fn fetch_identity_only(&self, id: &ProfileId, r: &RegionCode, ) -> Result<Option<Profile>> {
        self.fetch(id, r).await
    }
    async fn fetch_stats_only(&self, _: &ProfileId, _: &RegionCode, ) -> Result<Option<ProfileStats>> {
        Ok(None)
    }
    async fn save_identity(&self, p: &Profile, _original: Option<&Profile>, tx: Option<&mut dyn Transaction>) -> Result<()> {
        ProfileIdentityRepository::save(self, p, tx).await
    }
    async fn exists_by_handle(&self, h: &Handle, r: &RegionCode) -> Result<bool> {
        ProfileIdentityRepository::exists_by_handle(self, h, r).await
    }
    async fn delete_full_profile(&self, _id: &ProfileId, _r: &RegionCode) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
}