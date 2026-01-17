// crates/profile/src/infrastructure/repositories/postgres_identity_repository

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{RegionCode, AccountId, Username};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::SqlxErrorExt;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileIdentityRepository;
use crate::infrastructure::postgres::rows::PostgresProfileRow;

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
        let p = profile.clone();
        let current_version = p.version();

        <dyn Transaction>::execute_on(&pool, tx, |conn| Box::pin(async move {
            // 1. Tenter l'UPDATE avec Optimistic Locking
            let update_sql = r#"
                UPDATE user_profiles SET
                    display_name = $1, username = $2, bio = $3,
                    avatar_url = $4, banner_url = $5, location_label = $6,
                    social_links = $7, post_count = $8, is_private = $9,
                    updated_at = $10, version = version + 1
                WHERE account_id = $11 AND region_code = $12 AND version = $13
            "#;

            let result = sqlx::query(update_sql)
                .bind(p.display_name.as_str())
                .bind(p.username.as_str())
                .bind(p.bio.as_ref().map(|b| b.as_str()))
                .bind(p.avatar_url.as_ref().map(|u| u.as_str()))
                .bind(p.banner_url.as_ref().map(|u| u.as_str()))
                .bind(p.location_label.as_ref().map(|l| l.as_str()))
                .bind(serde_json::to_value(&p.social_links).unwrap_or_default())
                .bind(p.post_count.value() as i64)
                .bind(p.is_private)
                .bind(p.updated_at)
                .bind(p.account_id.as_uuid())
                .bind(p.region_code.as_str())
                .bind(current_version)
                .execute(&mut *conn)
                .await
                .map_domain::<Profile>()?;

            if result.rows_affected() == 0 {
                // 2. Si l'UPDATE a échoué, est-ce parce que le profil n'existe pas encore ?
                // On vérifie l'existence pour savoir s'il faut INSERT ou lever un ConcurrencyConflict
                let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM user_profiles WHERE account_id = $1 AND region_code = $2)")
                    .bind(p.account_id.as_uuid())
                    .bind(p.region_code.as_str())
                    .fetch_one(&mut *conn)
                    .await
                    .map_domain::<Profile>()?;

                if !exists.0 {
                    // INSERT initial
                    let insert_sql = r#"
                        INSERT INTO user_profiles (
                            account_id, region_code, display_name, username, version, updated_at, ...
                        ) VALUES ($1, $2, $3, $4, 1, NOW(), ...)
                    "#;
                    // ... exécuter l'insert ...
                    Ok(())
                } else {
                    // C'est un vrai conflit de version !
                    Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                        reason: format!("Profile version mismatch for user {}", p.account_id)
                    })
                }
            } else {
                Ok(())
            }
        })).await
    }

    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<Profile>> {
        let sql = "SELECT * FROM user_profiles WHERE account_id = $1 AND region_code = $2";

        let row = sqlx::query_as::<_, PostgresProfileRow>(sql)
            .bind(account_id.as_uuid())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Profile>()?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn find_by_username(&self, username: &Username, region: &RegionCode) -> Result<Option<Profile>> {
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
        let sql = "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE username = $1 AND region_code = $2)";

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
        <dyn Transaction>::execute_on(&pool, None, |conn| Box::pin(async move {
            let sql = "DELETE FROM user_profiles WHERE account_id = $1 AND region_code = $2";

            sqlx::query(sql)
                .bind(uid.as_uuid())
                .bind(reg.as_str())
                .execute(conn)
                .await
                .map_domain::<Profile>()
        })).await?;

        Ok(())
    }
}