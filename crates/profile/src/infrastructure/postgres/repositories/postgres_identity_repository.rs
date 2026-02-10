// crates/profile/src/infrastructure/postgres/repositories/postgres_identity_repository.rs

use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileIdentityRepository;
use crate::infrastructure::postgres::rows::PostgresProfileRow;
use crate::domain::value_objects::{ProfileId, Handle}; // Nouveaux imports
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{Result, DomainError};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::{PgPool, Row};

pub struct PostgresIdentityRepository {
    pool: PgPool,
}

impl PostgresIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProfileIdentityRepository for PostgresIdentityRepository {
    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let pool = self.pool.clone();
        let row = PostgresProfileRow::from(profile);

        <dyn Transaction>::execute_on(&pool, tx, |conn| Box::pin(async move {
            let old_version = row.version - 1;

            // 1. UPDATE basé sur profile_id + region_code (PK composite)
            let update_sql = r#"
                UPDATE user_profiles SET
                    owner_id = $1, display_name = $2, handle = $3, bio = $4,
                    avatar_url = $5, banner_url = $6, location_label = $7,
                    social_links = $8, post_count = $9, is_private = $10,
                    updated_at = $11, version = $12
                WHERE id = $13 AND region_code = $14 AND version = $15
            "#;

            let result = sqlx::query(update_sql)
                .bind(row.owner_id)       // $1
                .bind(&row.display_name)  // $2
                .bind(&row.handle)        // $3 (renommé)
                .bind(&row.bio)           // $4
                .bind(&row.avatar_url)    // $5
                .bind(&row.banner_url)    // $6
                .bind(&row.location_label)// $7
                .bind(&row.social_links)  // $8
                .bind(row.post_count)     // $9
                .bind(row.is_private)     // $10
                .bind(row.updated_at)     // $11
                .bind(row.version)        // $12
                .bind(row.id)             // $13 (PK part 1)
                .bind(&row.region_code)   // $14 (PK part 2)
                .bind(old_version)        // $15
                .execute(&mut *conn)
                .await
                .map_domain::<Profile>()?;

            if result.rows_affected() == 0 {
                // 2. Si rien n'est mis à jour, on vérifie si c'est un nouvel insert
                let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM user_profiles WHERE id = $1 AND region_code = $2)")
                    .bind(row.id)
                    .bind(&row.region_code)
                    .fetch_one(&mut *conn)
                    .await
                    .map_domain::<Profile>()?;

                if !exists {
                    // 3. INSERT initial
                    let insert_sql = r#"
                        INSERT INTO user_profiles (
                            id, owner_id, region_code, display_name, handle,
                            bio, avatar_url, banner_url, location_label,
                            social_links, post_count, is_private, version,
                            created_at, updated_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)
                    "#;

                    sqlx::query(insert_sql)
                        .bind(row.id)             // $1
                        .bind(row.owner_id)       // $2
                        .bind(&row.region_code)   // $3
                        .bind(&row.display_name)  // $4
                        .bind(&row.handle)        // $5
                        .bind(&row.bio)           // $6
                        .bind(&row.avatar_url)    // $7
                        .bind(&row.banner_url)    // $8
                        .bind(&row.location_label)// $9
                        .bind(&row.social_links)  // $10
                        .bind(row.post_count)     // $11
                        .bind(row.is_private)     // $12
                        .bind(row.version)        // $13
                        .bind(row.created_at)     // $14
                        .execute(&mut *conn)
                        .await
                        .map_domain::<Profile>()?;

                    Ok(())
                } else {
                    Err(DomainError::ConcurrencyConflict {
                        reason: format!("Profile version mismatch for ID {}", row.id)
                    })
                }
            } else {
                Ok(())
            }
        })).await
    }

    async fn fetch(&self, id: &ProfileId, region: &RegionCode) -> Result<Option<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE id = $1 AND region_code = $2";

        let row = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(id.as_uuid())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Profile>()?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn fetch_by_handle(&self, handle: &Handle, region: &RegionCode) -> Result<Option<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE handle = $1 AND region_code = $2";

        let row = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(handle.as_str())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Profile>()?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn fetch_all_by_owner(&self, owner_id: &AccountId) -> Result<Vec<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE owner_id = $1 ORDER BY created_at DESC";

        let rows = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(owner_id.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_domain::<Profile>()?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn exists_by_handle(&self, handle: &Handle, region: &RegionCode) -> Result<bool> {
        let sql = "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE handle = $1 AND region_code = $2)";

        let res = sqlx::query(sql)
            .bind(handle.as_str())
            .bind(region.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<Profile>()?;

        Ok(res.get(0))
    }

    async fn delete(&self, id: &ProfileId, region: &RegionCode) -> Result<()> {
        let sql = "DELETE FROM user_profiles WHERE id = $1 AND region_code = $2";

        sqlx::query(sql)
            .bind(id.as_uuid())
            .bind(region.as_str())
            .execute(&self.pool)
            .await
            .map_domain::<Profile>()?;

        Ok(())
    }
}