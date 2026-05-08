// crates/profile/src/infrastructure/postgres/repositories/postgres_identity_repository.rs

use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use crate::domain::value_objects::{Handle, ProfileId};
use crate::infrastructure::postgres::rows::PostgresProfileRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::Versioned;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
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

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            // 1. Verrouillage et lecture de la version actuelle pour OCC
            let db_v: Option<i64> = sqlx::query_scalar(
                "SELECT version FROM user_profiles WHERE profile_id = $1 AND region_code = $2 FOR UPDATE"
            )
            .bind(row.profile_id)
            .bind(&row.region_code)
            .fetch_optional(&mut *conn)
            .await
            .map_domain_infra("Profile: check version for save")?;

            match db_v {
                None => {
                    // 2. INSERT (Première création)
                    sqlx::query(
                        r#"INSERT INTO user_profiles (
                            profile_id, account_id, region_code, display_name, handle,
                            bio, avatar_url, banner_url, location_label,
                            social_links, is_private, version,
                            created_at, updated_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#
                    )
                    .bind(row.profile_id).bind(row.account_id).bind(&row.region_code)
                    .bind(&row.display_name).bind(&row.handle).bind(&row.bio)
                    .bind(&row.avatar_url).bind(&row.banner_url).bind(&row.location_label)
                    .bind(&row.social_links).bind(row.is_private).bind(next_version)
                    .bind(row.created_at).bind(row.updated_at)
                    .execute(&mut *conn)
                    .await
                    .map_domain_infra("Profile: insert")?;
                }
                Some(v) => {
                    // 3. UPDATE (OCC Check)
                    let current_version_expected = next_version - 1;
                    if v != current_version_expected {
                        return Err(DomainError::ConcurrencyConflict {
                            reason: format!("Profile {}: OCC mismatch (DB v{}, App expected v{})", row.profile_id, v, current_version_expected)
                        });
                    }

                    sqlx::query(
                        r#"UPDATE user_profiles SET
                            display_name = $1, handle = $2, bio = $3,
                            avatar_url = $4, banner_url = $5, location_label = $6,
                            social_links = $7, is_private = $8,
                            updated_at = $9, version = $10
                        WHERE profile_id = $11 AND region_code = $12"#
                    )
                    .bind(&row.display_name).bind(&row.handle).bind(&row.bio)
                    .bind(&row.avatar_url).bind(&row.banner_url).bind(&row.location_label)
                    .bind(&row.social_links).bind(row.is_private).bind(row.updated_at)
                    .bind(next_version).bind(row.profile_id).bind(&row.region_code)
                    .execute(&mut *conn)
                    .await
                    .map_domain_infra("Profile: update")?;
                }
            }
            Ok(())
        })).await?;

        // 4. Invalidation du cache (Après transaction réussie)
        let cache_handle = self.cache.clone();
        tokio::spawn(async move {
            let _ = cache_handle.delete(&key).await;
        });

        Ok(())
    }

    async fn find_by_id(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let key = Self::cache_key(id, region);
        let is_no_tx = tx.is_none();

        // 1. Essai Cache (Uniquement si hors transaction)
        if is_no_tx {
            if let Ok(Some(profile)) = self.cache.get_obj::<Profile>(&key).await {
                return Ok(Some(profile));
            }
        }

        // 2. Lecture DB via execute_on
        let uid = id.as_uuid();
        let r_str = region.as_str().to_string();

        let profile_opt = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let u = uid;
            let r = r_str.clone();
            Box::pin(async move {
                let sql = "SELECT * FROM user_profiles WHERE profile_id = $1 AND region_code = $2";
                let row_opt = sqlx::query_as::<_, PostgresProfileRow>(sql)
                    .bind(u)
                    .bind(r)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("Profile: fetch by id")?;

                row_opt.map(|r| r.to_domain()).transpose()
            })
        })
        .await?;

        // 3. Update Cache si lecture réussie hors transaction
        if is_no_tx {
            if let Some(ref p) = profile_opt {
                let cache_handle = self.cache.clone();
                let p_clone = p.clone();
                tokio::spawn(async move {
                    let _ = cache_handle
                        .set_obj(&key, &p_clone, Some(std::time::Duration::from_secs(600)))
                        .await;
                });
            }
        }

        Ok(profile_opt)
    }

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let is_no_tx = tx.is_none();
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
                    .map_domain_infra("Profile: fetch by handle")?;

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
                .map_domain_infra("Profile: fetch_all_by_account_id")?;

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
                    .map_domain_infra("Profile: delete")?;
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
}
