// crates/account/src/infrastructure/postgres/repositories/account_metadata_repository.rs

use async_trait::async_trait;
use sqlx::{Pool, Postgres, query, query_as};

use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;

use crate::domain::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;
use crate::infrastructure::postgres::models::PostgresAccountRole;
use crate::infrastructure::postgres::rows::PostgresAccountMetadataRow;

pub struct PostgresAccountMetadataRepository {
    pool: Pool<Postgres>,
}

impl PostgresAccountMetadataRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountMetadataRepository for PostgresAccountMetadataRepository {
    /// Récupère les métadonnées d'un compte.
    /// Charge également la colonne 'version' pour l'idempotence technique.
    async fn find_by_account_id(&self, account_id: &AccountId) -> Result<Option<AccountMetadata>> {
        let uid = account_id.as_uuid();

        let row = <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                let sql = "SELECT * FROM user_internal_metadata WHERE account_id = $1";
                query_as::<_, PostgresAccountMetadataRow>(sql)
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<AccountMetadata>()
            })
        })
        .await?;

        row.map(|r| AccountMetadata::try_from(r)).transpose()
    }

    /// Insertion initiale. La version est fixée à 1 (via metadata.version()).
    async fn insert(&self, metadata: &AccountMetadata, tx: &mut dyn Transaction) -> Result<()> {
        // 1. On prépare des versions "Owned" (Clonées ou copiées) de toutes les données
        let uid = metadata.account_id().as_uuid();
        let region = metadata.region_code().to_string();
        let role = PostgresAccountRole::from(metadata.role());
        let is_beta = metadata.is_beta_tester();
        let is_shadow = metadata.is_shadowbanned();
        let trust = metadata.trust_score();
        let notes = metadata.moderation_notes().map(|s| s.to_string());
        let ip = metadata.estimated_ip().map(|s| s.to_string());
        let updated = metadata.updated_at();
        let version = metadata.version();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            // 2. On utilise 'async move' pour transférer la propriété des variables ci-dessus
            Box::pin(async move {
                let sql = r#"
                INSERT INTO user_internal_metadata (
                    account_id, region_code, role, is_beta_tester,
                    is_shadowbanned, trust_score, moderation_notes,
                    estimated_ip, version, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#;

                query(sql)
                    .bind(uid)
                    .bind(region)
                    .bind(role)
                    .bind(is_beta)
                    .bind(is_shadow)
                    .bind(trust)
                    .bind(notes)
                    .bind(ip)
                    .bind(version)
                    .bind(updated)
                    .execute(conn)
                    .await
                    .map_domain::<AccountMetadata>()
            })
        })
            .await?;

        Ok(())
    }

    async fn save(
        &self,
        metadata: &AccountMetadata,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        // 1. Préparation des données Owned
        let uid = metadata.account_id().as_uuid();
        let current_version = metadata.version();
        let role = PostgresAccountRole::from(metadata.role());
        let is_beta = metadata.is_beta_tester();
        let is_shadow = metadata.is_shadowbanned();
        let trust = metadata.trust_score();
        let notes = metadata.moderation_notes().map(|s| s.to_string());
        let ip = metadata.estimated_ip().map(|s| s.to_string());
        let updated = metadata.updated_at();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
                UPDATE user_internal_metadata
                SET
                    role = $1,
                    is_beta_tester = $2,
                    is_shadowbanned = $3,
                    trust_score = $4,
                    moderation_notes = $5,
                    estimated_ip = $6,
                    updated_at = $7,
                    version = version + 1
                WHERE account_id = $8
                  AND version = $9
            "#;

                let result = query(sql)
                    .bind(role)
                    .bind(is_beta)
                    .bind(is_shadow)
                    .bind(trust)
                    .bind(notes)
                    .bind(ip)
                    .bind(updated)
                    .bind(uid)
                    .bind(current_version)
                    .execute(conn)
                    .await
                    .map_domain::<AccountMetadata>()?;

                if result.rows_affected() == 0 {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!(
                            "Metadata update failed: version mismatch (expected {})",
                            current_version
                        ),
                    });
                }

                Ok(())
            })
        })
            .await?;

        Ok(())
    }
}