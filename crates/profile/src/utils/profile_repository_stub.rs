// crates/profile/src/utils/test_utils.rs
#![cfg(test)]

use std::any::Any;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use futures::Future;

use crate::domain::entities::Profile;
use crate::domain::repositories::{ProfileIdentityRepository, ProfileRepository, ProfileStatsRepository};
use crate::domain::value_objects::ProfileStats;
use shared_kernel::domain::transaction::{Transaction, TransactionManager};
use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode, Counter};
use shared_kernel::domain::repositories::{OutboxRepository, CacheRepository};
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::errors::{Result, AppResult, AppError, ErrorCode};

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
    async fn find_by_id(&self, _id: &AccountId, _r: &RegionCode) -> Result<Option<Profile>> {
        Ok(self.profile_to_return.lock().unwrap().clone())
    }
    async fn find_by_username(&self, _: &Username, _: &RegionCode) -> Result<Option<Profile>> {
        Ok(self.profile_to_return.lock().unwrap().clone())
    }
    async fn exists_by_username(&self, _u: &Username, _r: &RegionCode) -> Result<bool> {
        Ok(*self.exists_return.lock().unwrap())
    }
    async fn delete_identity(&self, _: &AccountId, _: &RegionCode) -> Result<()> { Ok(()) }
}

#[async_trait::async_trait]
impl ProfileRepository for ProfileRepositoryStub {
    async fn get_profile_by_account_id(&self, id: &AccountId, r: &RegionCode) -> Result<Option<Profile>> {
        self.find_by_id(id, r).await
    }
    async fn get_full_profile_by_username(&self, username: &Username, region: &RegionCode) -> Result<Option<Profile>> {
        self.find_by_username(username, region).await
    }
    async fn get_profile_without_stats(&self, id: &AccountId, r: &RegionCode) -> Result<Option<Profile>> {
        self.find_by_id(id, r).await
    }
    async fn get_profile_stats(&self, _: &AccountId, _: &RegionCode) -> Result<Option<ProfileStats>> {
        Ok(None) // Généralement géré par le StatsRepoStub maintenant
    }
    async fn save(&self, p: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
        ProfileIdentityRepository::save(self, p, tx).await
    }
    async fn exists_by_username(&self, u: &Username, r: &RegionCode) -> Result<bool> {
        ProfileIdentityRepository::exists_by_username(self, u, r).await
    }
}

// --- STUB OUTBOX ---
pub struct OutboxRepoStub;
#[async_trait::async_trait]
impl OutboxRepository for OutboxRepoStub {
    async fn save(&self, _tx: &mut dyn Transaction, _event: &dyn DomainEvent) -> Result<()> { Ok(()) }
}

// --- TRANSACTION MANAGEMENT ---
pub struct FakeTransaction;
impl Transaction for FakeTransaction {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

pub struct StubTxManager;
impl TransactionManager for StubTxManager {
    fn in_transaction<'a>(
        &'a self,
        f: Box<dyn FnOnce(Box<dyn Transaction>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { f(Box::new(FakeTransaction)).await })
    }
}