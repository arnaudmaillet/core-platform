// crates/profile/src/infrastructure/postgres/repositories/postgres_identity_repository.rs

use crate::entities::Profile;
use crate::infrastructure::postgres::mappers::PostgresProfileRow;
use crate::repositories::ProfileRepository;
use crate::types::Handle;
use async_trait::async_trait;
use infra_sqlx::TransactionExecuteExt;
use infra_sqlx::{sqlx, sqlx::PgPool};
use shared_kernel::core::{ManagedEntity, Error, Identifier, Result, Transaction, Versioned};
use shared_kernel::types::{AccountId, ProfileId, Region};

const OCC_LOCK_QUERY: &str = r#"
    SELECT version FROM user_profiles WHERE profile_id = $1 AND region = $2 FOR UPDATE
"#;

const UPDATE_PROFILE_QUERY: &str = r#"
    UPDATE user_profiles 
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
      AND region = $12 
      AND version = $13
"#;

const INSERT_PROFILE_QUERY: &str = r#"
    INSERT INTO user_profiles (profile_id, account_id, region, display_name, handle, bio, avatar_url, banner_url, location_label, social_links, is_private, version, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
"#;

const FIND_BY_ID_QUERY: &str = r#"
    SELECT * FROM user_profiles WHERE profile_id = $1 AND region = $2
"#;

const FIND_BY_HANDLE_QUERY: &str = r#"
    SELECT * FROM user_profiles WHERE handle = $1 AND region = $2
"#;

const FIND_ALL_BY_ACCOUNT_ID_QUERY: &str = r#"
    SELECT * FROM user_profiles WHERE account_id = $1 AND region = $2 ORDER BY created_at DESC
"#;

const DELETE_PROFILE_QUERY: &str = r#"
    DELETE FROM user_profiles WHERE profile_id = $1 AND region = $2
"#;

const EXISTS_QUERY: &str = r#"
    SELECT EXISTS(SELECT 1 FROM user_profiles WHERE profile_id = $1 AND region = $2)
"#;

const EXISTS_BY_HANDLE_QUERY: &str = r#"
    SELECT EXISTS(SELECT 1 FROM user_profiles WHERE handle = $1 AND region = $2)
"#;

pub struct PostgresProfileRepository {
    pool: PgPool,
}

impl PostgresProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProfileRepository for PostgresProfileRepository {
    async fn save(
        &self,
        region: Region,
        profile: &mut Profile,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let row = PostgresProfileRow::from_domain(profile);
        let next_version = profile.version() as i64;
        let region_str = region.as_str().to_string();
        let is_events_empty = profile.lifecycle().is_events_empty();

        let result = self
            .pool
            .execute_on(tx, |conn| {
                let r_str = region_str.clone();
                Box::pin(async move {
                    // 1. ÉVALUATION DE L'IDEMPOTENCE & LOCK OCC
                    let current_db_v: Option<i64> = sqlx::query_scalar(OCC_LOCK_QUERY)
                        .bind(row.profile_id)
                        .bind(&r_str)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(|e| {
                            Error::database(format!("Profile save repository lock failed: {}", e))
                        })?;

                    match current_db_v {
                        Some(v) => {
                            // --- MODE UPDATE (OCC) ---
                            let is_noop = next_version == v && is_events_empty;

                            if is_noop {
                                return Ok(());
                            }

                            let current_version_expected = next_version - 1;
                            if v != current_version_expected {
                                return Err(Error::concurrency_conflict(format!(
                                    "Profile {}: OCC mismatch (DB v{}, App expected v{})",
                                    row.profile_id, v, current_version_expected
                                )));
                            }

                            sqlx::query(UPDATE_PROFILE_QUERY)
                                .bind(&row.display_name)
                                .bind(&row.handle)
                                .bind(&row.bio)
                                .bind(&row.avatar_url)
                                .bind(&row.banner_url)
                                .bind(&row.location_label)
                                .bind(&row.social_links)
                                .bind(row.is_private)
                                .bind(row.updated_at)
                                .bind(next_version)
                                .bind(row.profile_id)
                                .bind(&r_str)
                                .bind(v)
                                .execute(&mut *conn)
                                .await
                                .map_err(|e| {
                                    Error::database(format!("Profile update failed: {}", e))
                                })?;
                        }
                        None => {
                            // --- MODE INSERT ---
                            sqlx::query(INSERT_PROFILE_QUERY)
                                .bind(row.profile_id)
                                .bind(row.account_id)
                                .bind(&r_str)
                                .bind(&row.display_name)
                                .bind(&row.handle)
                                .bind(&row.bio)
                                .bind(&row.avatar_url)
                                .bind(&row.banner_url)
                                .bind(&row.location_label)
                                .bind(&row.social_links)
                                .bind(row.is_private)
                                .bind(next_version)
                                .bind(row.created_at)
                                .bind(row.updated_at)
                                .execute(&mut *conn)
                                .await
                                .map_err(|e| {
                                    Error::database(format!("Profile insert failed: {}", e))
                                })?;
                        }
                    }

                    Ok(())
                })
            })
            .await;

        result
    }

    async fn find_by_id(
        &self,
        id: ProfileId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let pid = id.as_uuid();
        let r_str = region.as_str().to_string();

        let profile_opt = self
            .pool
            .execute_on(tx, |conn| {
                let uid = pid;
                let r = r_str.clone();
                Box::pin(async move {
                    let row_opt = sqlx::query_as::<_, PostgresProfileRow>(FIND_BY_ID_QUERY)
                        .bind(uid)
                        .bind(r)
                        .fetch_optional(conn)
                        .await
                        .map_err(|e| {
                            Error::database(format!("Profile fetch_by_id repository: {}", e))
                        })?;

                    row_opt.map(|r| r.to_domain()).transpose()
                })
            })
            .await?;

        Ok(profile_opt)
    }

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let handle_str = handle.as_str().to_string();
        let region_str = region.as_str().to_string();

        self.pool
            .execute_on(tx, |conn| {
                let h = handle_str.clone();
                let r = region_str.clone();
                Box::pin(async move {
                    let row_opt = sqlx::query_as::<_, PostgresProfileRow>(FIND_BY_HANDLE_QUERY)
                        .bind(h)
                        .bind(r)
                        .fetch_optional(conn)
                        .await
                        .map_err(|e| {
                            Error::database(format!("Profile fetch_by_handle repository: {}", e))
                        })?;

                    row_opt.map(|r| r.to_domain()).transpose()
                })
            })
            .await
    }

    async fn find_all_by_account_id(
        &self,
        account_id: AccountId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>> {
        let uid = account_id.uuid();
        let r_str = region.as_str().to_string();

        self.pool
            .execute_on(tx, |conn| {
                let u = uid;
                let r = r_str.clone();
                Box::pin(async move {
                    let rows =
                        sqlx::query_as::<_, PostgresProfileRow>(FIND_ALL_BY_ACCOUNT_ID_QUERY)
                            .bind(u)
                            .bind(r)
                            .fetch_all(conn)
                            .await
                            .map_err(|e| {
                                Error::database(format!(
                                    "Profile find_all_by_account_id repository: {}",
                                    e
                                ))
                            })?;

                    rows.into_iter().map(|r| r.to_domain()).collect()
                })
            })
            .await
    }

    async fn delete(
        &self,
        id: ProfileId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = id.as_uuid();
        let r_str = region.as_str().to_string();

        self.pool
            .execute_on(tx, |conn| {
                let u = uid;
                let r = r_str.clone();
                Box::pin(async move {
                    sqlx::query(DELETE_PROFILE_QUERY)
                        .bind(u)
                        .bind(r)
                        .execute(conn)
                        .await
                        .map_err(|e| {
                            Error::database(format!("Profile delete repository failed: {}", e))
                        })?;
                    Ok(())
                })
            })
            .await?;

        Ok(())
    }

    async fn exists(&self, profile_id: ProfileId, region: Region) -> Result<bool> {
        let uid = profile_id.as_uuid();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(EXISTS_QUERY)
            .bind(uid)
            .bind(r_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::database(format!("Profile exists repository failed: {}", e)))?;

        Ok(exists)
    }

    async fn exists_by_handle(&self, handle: &Handle, region: Region) -> Result<bool> {
        let h_str = handle.as_str().to_string();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(EXISTS_BY_HANDLE_QUERY)
            .bind(h_str)
            .bind(r_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                Error::database(format!("Profile exists_by_handle repository failed: {}", e))
            })?;

        Ok(exists)
    }
}
