// crates/profile/src/infrastructure/repositories/postgres_identity_repository

use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileIdentityRepository;
use crate::infrastructure::postgres::rows::PostgresProfileRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::{PgPool, Row};

pub struct PostgresProfileRepository {
    pool: PgPool,
}

impl PostgresProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProfileIdentityRepository for PostgresProfileRepository {
    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let pool = self.pool.clone();
        // On prépare la Row en amont (conversion infaillible From)
        let row = PostgresProfileRow::from(profile);

        <dyn Transaction>::execute_on(&pool, tx, |conn| Box::pin(async move {
            // 1. On calcule l'ancienne version (celle avant l'incrément du domaine)
            let current_version = row.version; // C'est la version N + 1
            let old_version = current_version - 1;

            // 2. Tentative d'UPDATE (Optimistic Locking)
            let update_sql = r#"
                UPDATE user_profiles SET
                    display_name = $1, username = $2, bio = $3,
                    avatar_url = $4, banner_url = $5, location_label = $6,
                    social_links = $7, post_count = $8, is_private = $9,
                    updated_at = $10,
                    version = $13  -- On enregistre la nouvelle version (déjà incrémentée)
                WHERE account_id = $11 AND region_code = $12 AND version = $14 -- On check l'ancienne
            "#;

            let result = sqlx::query(update_sql)
                .bind(&row.display_name)
                .bind(&row.username)
                .bind(&row.bio)
                .bind(&row.avatar_url)
                .bind(&row.banner_url)
                .bind(&row.location_label)
                .bind(&row.social_links)
                .bind(row.post_count)
                .bind(row.is_private)
                .bind(row.updated_at)
                .bind(row.account_id)
                .bind(&row.region_code)
                .bind(row.version) // nouvelle version (ex: 2)
                .bind(old_version) // ancienne version attendue en DB (ex: 1)
                .execute(&mut *conn)
                .await
                .map_domain::<Profile>()?;

            if result.rows_affected() == 0 {
                // 3. Si l'UPDATE échoue, on vérifie si le profil existe
                let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM user_profiles WHERE account_id = $1 AND region_code = $2)")
                    .bind(row.account_id)
                    .bind(&row.region_code)
                    .fetch_one(&mut *conn)
                    .await
                    .map_domain::<Profile>()?;

                if !exists {
                    // 4. INSERT initial
                    let insert_sql = r#"
                INSERT INTO user_profiles (
                    account_id, region_code, display_name, username,
                    bio, avatar_url, banner_url, location_label,
                    social_links, post_count, is_private, version,
                    created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)
                "#;

                    sqlx::query(insert_sql)
                        .bind(row.account_id)
                        .bind(&row.region_code)
                        .bind(&row.display_name)
                        .bind(&row.username)
                        .bind(&row.bio)
                        .bind(&row.avatar_url)
                        .bind(&row.banner_url)
                        .bind(&row.location_label)
                        .bind(&row.social_links)
                        .bind(row.post_count)
                        .bind(row.is_private)
                        .bind(row.version)
                        .bind(row.created_at)
                        .execute(&mut *conn)
                        .await
                        .map_domain::<Profile>()?;

                    Ok(())
                } else {
                    // 5. Conflit de concurrence : le profil existe mais la version a changé entre-temps
                    Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                        reason: format!("Profile version mismatch for account {}", row.account_id)
                    })
                }
            } else {
                Ok(())
            }
        })).await
    }

    async fn find_by_id(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
    ) -> Result<Option<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE account_id = $1 AND region_code = $2";

        let row = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(account_id.as_uuid())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Profile>()?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn find_by_username(
        &self,
        username: &Username,
        region: &RegionCode,
    ) -> Result<Option<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE username = $1 AND region_code = $2";

        let row = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(username.as_str())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Profile>()?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn exists_by_username(&self, username: &Username, region: &RegionCode) -> Result<bool> {
        let sql =
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE username = $1 AND region_code = $2)";

        let row = sqlx::query(sql)
            .bind(username.as_str())
            .bind(region.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<Profile>()?;

        Ok(row.get(0))
    }

    async fn delete_identity(&self, account_id: &AccountId, region: &RegionCode) -> Result<()> {
        let pool = self.pool.clone();
        let uid = *account_id;
        let reg = region.clone();

        // Suppression transactionnelle (au cas où on ajoute des logs d'audit plus tard)
        <dyn Transaction>::execute_on(&pool, None, |conn| {
            Box::pin(async move {
                let sql = "DELETE FROM user_profiles WHERE account_id = $1 AND region_code = $2";

                sqlx::query(sql)
                    .bind(uid.as_uuid())
                    .bind(reg.as_str())
                    .execute(conn)
                    .await
                    .map_domain::<Profile>()
            })
        })
        .await?;

        Ok(())
    }
}
