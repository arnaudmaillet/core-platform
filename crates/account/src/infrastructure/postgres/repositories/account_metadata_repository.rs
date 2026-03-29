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
    async fn fetch_by_account_id(&self, account_id: &AccountId) -> Result<Option<AccountMetadata>> {
        let uid = account_id.as_uuid();
        let row = <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                let sql = "SELECT * FROM account_metadata WHERE account_id = $1";
                query_as::<_, PostgresAccountMetadataRow>(sql)
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("AccountMetadata: fetch")
            })
        })
        .await?;

        row.map(AccountMetadata::try_from).transpose()
    }

   async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        // --- 1. EXTRACTION DES DONNÉES (Propriété complète / Owned types) ---
        let uid = metadata.account_id().as_uuid();
        let region = metadata.region_code().to_string();
        let role = PostgresAccountRole::from(metadata.role());
        let is_beta = metadata.is_beta_tester();
        let is_shadow = metadata.is_shadowbanned();
        let trust = metadata.trust_score();
        let notes = metadata.moderation_notes().map(|s| s.to_string());
        let ip = metadata.estimated_ip().map(|s| s.to_string());
        let last_mod = metadata.last_moderation_at();
        let updated = metadata.updated_at();
        let new_version = metadata.version_i64()?;

        let old_region = original.map(|o| o.region_code().to_string());

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                // --- ÉTAPE 1 : GESTION DU CHANGEMENT DE RÉGION ---
                if let Some(old_reg) = old_region {
                    if old_reg != region {
                        query("DELETE FROM account_metadata WHERE account_id = $1 AND region_code = $2")
                            .bind(uid)
                            .bind(old_reg)
                            .execute(&mut *conn)
                            .await
                            .map_domain_infra("AccountMetadata: delete old region")?;
                    }
                }

                // --- ÉTAPE 2 : UPSERT ATOMIQUE ---
                let sql = r#"
                    INSERT INTO account_metadata (
                        account_id, region_code, role, is_beta_tester, is_shadowbanned,
                        trust_score, moderation_notes, estimated_ip, last_moderation_at,
                        version, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                    ON CONFLICT (account_id, region_code) DO UPDATE SET
                        role = EXCLUDED.role,
                        is_beta_tester = EXCLUDED.is_beta_tester,
                        is_shadowbanned = EXCLUDED.is_shadowbanned,
                        trust_score = EXCLUDED.trust_score,
                        moderation_notes = EXCLUDED.moderation_notes,
                        estimated_ip = EXCLUDED.estimated_ip,
                        last_moderation_at = EXCLUDED.last_moderation_at,
                        version = EXCLUDED.version,
                        updated_at = EXCLUDED.updated_at
                    WHERE account_metadata.version < EXCLUDED.version
                "#;

                let result = query(sql)
                    .bind(uid)              // $1
                    .bind(&region)          // $2
                    .bind(role)             // $3
                    .bind(is_beta)          // $4
                    .bind(is_shadow)        // $5
                    .bind(trust)            // $6
                    .bind(notes)            // $7
                    .bind(ip)               // $8
                    .bind(last_mod)         // $9
                    .bind(new_version)      // $10
                    .bind(updated)          // $11
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountMetadata: save upsert")?;

                if result.rows_affected() == 0 {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!("OCC Conflict for {}: version in DB is already >= v{}", uid, new_version),
                    });
                }

                Ok(())
            })
        }).await
    }
}
