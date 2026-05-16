// crates/profile/src/infrastructure/postgres/repositories/postgres_identity_repository.rs

use crate::entities::Profile;
use crate::infrastructure::postgres::rows::PostgresProfileRow;
use crate::repositories::ProfileRepository;
use crate::types::{Handle, ProfileId};
use async_trait::async_trait;
use shared_kernel::cache::{CacheRepository, CacheRepositoryExt};
use shared_kernel::core::{Error, Identifier, Result, Transaction, Versioned};
use shared_kernel::types::{AccountId, RegionCode};
use sqlx::PgPool;
use std::sync::Arc;

pub struct PostgresProfileRepository {
    pool: PgPool,
    cache: Arc<dyn CacheRepository>,
}

impl PostgresProfileRepository {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache }
    }

    pub fn cache_key(profile_id: &ProfileId, region: &RegionCode) -> String {
        format!(
            "profile:aggregate:{}:{}",
            region.as_str(),
            profile_id.as_uuid()
        )
    }
}

#[async_trait]
impl ProfileRepository for PostgresProfileRepository {
    async fn save(&self, profile: &mut Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let row = PostgresProfileRow::from_domain(profile);
        let key = Self::cache_key(profile.profile_id(), profile.account_id().region());
        let next_version = profile.version() as i64;
        let expected_db_version: i64 = next_version - 1;

        let result = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
        Box::pin(async move {

    let res = sqlx::query(
    "UPDATE user_profiles 
     SET display_name = $1, 
         handle = $2, 
         bio = $3, 
         avatar_url = $4, 
         banner_url = $5, 
         location_label = $6, 
         social_links = $7, 
         is_private = $8, 
         updated_at = $9, 
         version = $10
     WHERE profile_id = $11 
       AND region_code = $12 
       AND version = $13"
)
.bind(&row.display_name)   // $1
.bind(&row.handle)         // $2
.bind(&row.bio)            // $3
.bind(&row.avatar_url)     // $4
.bind(&row.banner_url)     // $5
.bind(&row.location_label) // $6
.bind(&row.social_links)   // $7
.bind(row.is_private)      // $8
.bind(row.updated_at)      // $9
.bind(next_version)        // $10
.bind(row.profile_id)      // $11
.bind(&row.region_code)    // $12
.bind(expected_db_version) // $13
.execute(&mut *conn)
.await;

    let result = res.map_err(|e| Error::database(e.to_string()))?;
            if result.rows_affected() == 0 {
                let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_profiles WHERE profile_id = $1)")
                    .bind(row.profile_id)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|e| Error::database(format!("Profile save repository: {}", e.to_string())))?;

                if exists {
                    return Err(Error::concurrency_conflict(
                        format!("Profile {}: version mismatch", row.profile_id),
                    ));
                }

                sqlx::query(
                    r#"INSERT INTO user_profiles (profile_id, account_id, region_code, display_name, handle, bio, avatar_url, banner_url, location_label, social_links, is_private, version, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
                )
                .bind(row.profile_id).bind(row.account_id).bind(&row.region_code).bind(&row.display_name).bind(&row.handle).bind(&row.bio).bind(&row.avatar_url).bind(&row.banner_url).bind(&row.location_label).bind(&row.social_links).bind(row.is_private).bind(next_version).bind(row.created_at).bind(row.updated_at)
                .execute(&mut *conn)
                .await
                .map_err(|e| Error::database(format!("Profile save repository: {}", e.to_string())))?;
            }
            Ok(())
        })
    })
    .await;

        let cache_handle = self.cache.clone();
        tokio::spawn(async move {
            let _ = cache_handle.delete(&key).await;
        });

        result
    }

    async fn find_by_id(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let pid = id.as_uuid();
        let key = Self::cache_key(id, region);
        let is_no_tx = tx.is_none();

        // 1. Essai Cache
        if is_no_tx {
            let cache_result = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                self.cache.get_obj::<Profile>(&key),
            )
            .await;

            if let Ok(Ok(Some(profile))) = cache_result {
                return Ok(Some(profile));
            }
        }

        // 2. Lecture DB
        let profile_opt = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let uid = pid;
            let r_str = region.as_str().to_string();
            Box::pin(async move {
                let row_opt = sqlx::query_as::<_, PostgresProfileRow>(
                    "SELECT * FROM user_profiles WHERE profile_id = $1 AND region_code = $2",
                )
                .bind(uid)
                .bind(r_str)
                .fetch_optional(conn)
                .await
                .map_err(|e| {
                    Error::database(format!("Profile fetch_by_id repository: {}", e.to_string()))
                })?;

                row_opt.map(|r| r.to_domain()).transpose()
            })
        })
        .await?;

        // 3. MISE À JOUR DU CACHE
        if is_no_tx && profile_opt.is_some() {
            let cache_handle = self.cache.clone();
            let p_clone = profile_opt.clone().unwrap();
            let key_owned = key.clone();
            tokio::spawn(async move {
                // On garde un TTL de 10 min par exemple
                let _ = cache_handle
                    .set_obj(
                        &key_owned,
                        &p_clone,
                        Some(std::time::Duration::from_secs(600)),
                    )
                    .await;
            });
        }

        Ok(profile_opt)
    }

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let handle_str = handle.as_str().to_string();
        let region_str = region.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let h = handle_str.clone();
            let r = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT * FROM user_profiles WHERE handle = $1 AND region_code = $2";
                let row_opt = sqlx::query_as::<_, PostgresProfileRow>(sql)
                    .bind(h)
                    .bind(r)
                    .fetch_optional(conn)
                    .await
                    .map_err(|e| {
                        Error::database(format!(
                            "Profile fetch_by_handle repository: {}",
                            e.to_string()
                        ))
                    })?;

                row_opt.map(|r| r.to_domain()).transpose()
            })
        })
        .await
    }

    async fn find_all_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>> {
        let uid = account_id.uuid().clone();
        let r_str = account_id.region().as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
        let u = uid;
        let r = r_str.clone();
        Box::pin(async move {
            let sql = "SELECT * FROM user_profiles WHERE account_id = $1 AND region_code = $2 ORDER BY created_at DESC";
            let rows = sqlx::query_as::<_, PostgresProfileRow>(sql)
                .bind(u)
                .bind(r)
                .fetch_all(conn)
                .await
                .map_err(|e| Error::database(format!("Profile find_all_by_account_id repository: {}", e.to_string())))?;

            rows.into_iter().map(|r| r.to_domain()).collect()
        })
    }).await
    }

    async fn delete(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let key = Self::cache_key(id, region);
        let uid = id.as_uuid();
        let r_str = region.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let u = uid;
            let r = r_str.clone();
            Box::pin(async move {
                sqlx::query("DELETE FROM user_profiles WHERE profile_id = $1 AND region_code = $2")
                    .bind(u)
                    .bind(r)
                    .execute(conn)
                    .await
                    .map_err(|e| {
                        Error::database(format!("Profile delete repository: {}", e.to_string()))
                    })?;
                Ok(())
            })
        })
        .await?;

        // Invalidation du cache uniquement après la réussite de la transaction
        let cache_handle = self.cache.clone();
        tokio::spawn(async move {
            let _ = cache_handle.delete(&key).await;
        });

        Ok(())
    }

    async fn exists(&self, profile_id: &ProfileId, region: &RegionCode) -> Result<bool> {
        let uid = profile_id.as_uuid();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE profile_id = $1 AND region_code = $2)",
        )
        .bind(uid)
        .bind(r_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Profile exists repository: {}", e)))?;

        Ok(exists)
    }

    async fn exists_by_handle(&self, handle: &Handle, region: &RegionCode) -> Result<bool> {
        let h_str = handle.as_str().to_string();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE handle = $1 AND region_code = $2)",
        )
        .bind(h_str)
        .bind(r_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Profile exists_by_handle repository: {}", e)))?;

        Ok(exists)
    }
}
