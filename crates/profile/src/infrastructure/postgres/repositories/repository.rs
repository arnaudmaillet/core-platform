// crates/profile/src/infrastructure/postgres/repositories/postgres_identity_repository.rs

use crate::entities::Profile;
use crate::infrastructure::postgres::rows::PostgresProfileRow;
use crate::repositories::ProfileRepository;
use crate::types::{Handle};
use infra_sqlx::TransactionExecuteExt;
use async_trait::async_trait;
use shared_kernel::core::{AggregateRoot, Error, Identifier, Result, Transaction, Versioned};
use shared_kernel::types::{AccountId, Region, ProfileId};
use infra_sqlx::{sqlx, sqlx::PgPool};

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
    async fn save(&self, profile: &mut Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
    let row = PostgresProfileRow::from_domain(profile);
    let next_version = profile.version() as i64;
    
    // 1. EXTRACTION DU FLAG AVANT LA CLOSURE ASYNC (Lifetime Safe)
    let is_events_empty = profile.metadata().is_events_empty();

    let result = self.pool.execute_on(tx, |conn| {
        Box::pin(async move {
            // 2. ÉVALUATION DE L'IDEMPOTENCE
            let current_db_v: Option<i64> = sqlx::query_scalar(
                "SELECT version FROM user_profiles WHERE profile_id = $1 AND region = $2 FOR UPDATE"
            )
            .bind(row.profile_id)
            .bind(&row.region)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| Error::database(format!("Profile save repository: {}", e.to_string())))?;

            match current_db_v {
                Some(v) => {
                    // --- MODE UPDATE (OCC) ---
                    let is_noop = next_version == v && is_events_empty;
                    
                    if is_noop {
                        // C'est une écriture blanche d'idempotence technique, on court-circuite proprement !
                        return Ok(());
                    }

                    let current_version_expected = next_version - 1;
                    if v != current_version_expected {
                        return Err(Error::concurrency_conflict(
                            format!("Profile {}: OCC mismatch (DB v{}, App expected v{})", row.profile_id, v, current_version_expected),
                        ));
                    }

                    // On applique l'update standard puisque les versions matchent
                    sqlx::query(
                        r#"UPDATE user_profiles 
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
                             AND version = $13"# // 💡 FIX : Ordre strict $11, $12, $13
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
                    .bind(&row.region)    // $12
                    .bind(v)                   // $13 (L'ancienne version lue sous verrou)
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| Error::database(format!("Profile update failed: {}", e.to_string())))?;
                }
                None => {
                    // --- MODE INSERT ---
                    sqlx::query(
                        r#"INSERT INTO user_profiles (profile_id, account_id, region, display_name, handle, bio, avatar_url, banner_url, location_label, social_links, is_private, version, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
                    )
                    .bind(row.profile_id).bind(row.account_id).bind(&row.region).bind(&row.display_name).bind(&row.handle).bind(&row.bio).bind(&row.avatar_url).bind(&row.banner_url).bind(&row.location_label).bind(&row.social_links).bind(row.is_private).bind(next_version).bind(row.created_at).bind(row.updated_at)
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| Error::database(format!("Profile insert failed: {}", e.to_string())))?;
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
        // 2. Lecture DB
        let profile_opt = self.pool.execute_on(tx, |conn| {
            let uid = pid;
            let r_str = region.as_str().to_string();
            Box::pin(async move {
                let row_opt = sqlx::query_as::<_, PostgresProfileRow>(
                    "SELECT * FROM user_profiles WHERE profile_id = $1 AND region = $2",
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

        self.pool.execute_on(tx, |conn| {
            let h = handle_str.clone();
            let r = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT * FROM user_profiles WHERE handle = $1 AND region = $2";
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
        account_id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>> {
        let uid = account_id.uuid().clone();
        let r_str = account_id.region().as_str().to_string();

        self.pool.execute_on(tx, |conn| {
        let u = uid;
        let r = r_str.clone();
        Box::pin(async move {
            let sql = "SELECT * FROM user_profiles WHERE account_id = $1 AND region = $2 ORDER BY created_at DESC";
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
        id: ProfileId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = id.as_uuid();
        let r_str = region.as_str().to_string();

        self.pool.execute_on(tx, |conn| {
            let u = uid;
            let r = r_str.clone();
            Box::pin(async move {
                sqlx::query("DELETE FROM user_profiles WHERE profile_id = $1 AND region = $2")
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

        Ok(())
    }

    async fn exists(&self, profile_id: ProfileId, region: Region) -> Result<bool> {
        let uid = profile_id.as_uuid();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE profile_id = $1 AND region = $2)",
        )
        .bind(uid)
        .bind(r_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Profile exists repository: {}", e)))?;

        Ok(exists)
    }

    async fn exists_by_handle(&self, handle: &Handle, region: Region) -> Result<bool> {
        let h_str = handle.as_str().to_string();
        let r_str = region.as_str().to_string();

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE handle = $1 AND region = $2)",
        )
        .bind(h_str)
        .bind(r_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Profile exists_by_handle repository: {}", e)))?;

        Ok(exists)
    }
}
