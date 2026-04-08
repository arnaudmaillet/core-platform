// crates/account/src/infrastructure/postgres/repositories/account_metadata_repository.rs

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::{Pool, Postgres, query, query_as};

use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};

use crate::domain::account::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;
use crate::infrastructure::postgres::models::PostgresAccountRole;
use crate::infrastructure::postgres::rows::PostgresAccountMetadataRow;

pub struct PostgresAccountMetadataRepository {
    pool: Pool<Postgres>,
    cache: Arc<dyn CacheRepository>,
}

impl PostgresAccountMetadataRepository {
    pub fn new(pool: Pool<Postgres>, cache: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache }
    }

    fn cache_key(account_id: &AccountId) -> String {
        format!("account:metadata:{}", account_id.as_uuid())
    }
}

#[async_trait]
impl AccountMetadataRepository for PostgresAccountMetadataRepository {
    async fn fetch_by_account_id(&self, account_id: &AccountId) -> Result<Option<AccountMetadata>> {
        let key = Self::cache_key(account_id);

        // 1. TENTATIVE CACHE (Read-through)
        if let Ok(Some(metadata)) = self.cache.get_obj::<AccountMetadata>(&key).await {
            return Ok(Some(metadata));
        }

        // 2. LECTURE SQL
        let uid = account_id.as_uuid();
        // Utilisation de .as_deref_mut() pour éviter le move
        let row = <dyn Transaction>::execute_on(&self.pool, tx.as_deref_mut(), |conn| {
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

        let metadata = row.map(AccountMetadata::try_from).transpose()?;

        // 3. MISE EN CACHE (Write-through)
        if let Some(ref meta) = metadata {
            // TTL de 30 minutes pour les metadata (souvent moins critiques que l'Identity)
            let _ = self.cache.set_obj(&key, meta, Some(Duration::from_secs(1800))).await;
        }

        Ok(metadata)
    }

    async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        // --- 1. EXTRACTION DES DONNÉES ---
        let uid = metadata.account_id().as_uuid();
        let role = PostgresAccountRole::from(metadata.role());
        let is_beta = metadata.is_beta_tester();
        let is_shadow = metadata.is_shadowbanned();
        let trust = metadata.trust_score();
        let notes = metadata.moderation_notes().map(|s| s.to_string());
        let last_ip_addr = metadata.last_ip_addr().map(|ip| ip.to_std());
        let last_mod = metadata.last_moderation_at();
        let updated = metadata.updated_at();
        
        let new_version = metadata.version_i64()?;
        
        let old_version = original
            .map(|o| o.version_i64()).transpose()?.unwrap_or(0);

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                // --- ÉTAPE 1 : UPSERT ATOMIQUE ---
                let sql = r#"
                INSERT INTO account_metadata (
                    account_id, role, is_beta_tester, is_shadowbanned,
                    trust_score, moderation_notes, last_ip_addr, last_moderation_at,
                    version, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (account_id) DO UPDATE SET
                    role = EXCLUDED.role,
                    is_beta_tester = EXCLUDED.is_beta_tester,
                    is_shadowbanned = EXCLUDED.is_shadowbanned,
                    trust_score = EXCLUDED.trust_score,
                    moderation_notes = EXCLUDED.moderation_notes,
                    last_ip_addr = EXCLUDED.last_ip_addr,
                    last_moderation_at = EXCLUDED.last_moderation_at,
                    version = EXCLUDED.version,
                    updated_at = EXCLUDED.updated_at
                WHERE account_metadata.version = $11
            "#;

                let result = query(sql)
                    .bind(uid) // $1
                    .bind(role) // $2
                    .bind(is_beta) // $3
                    .bind(is_shadow) // $4
                    .bind(trust) // $5
                    .bind(notes) // $6
                    .bind(last_ip_addr) // $7
                    .bind(last_mod) // $8
                    .bind(new_version) // $9
                    .bind(updated) // $10
                    .bind(old_version) // $11
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountMetadata: save upsert")?;

                if result.rows_affected() == 0 && old_version > 0 {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!(
                            "OCC Conflict for metadata {}: expected v{}, but DB version has changed",
                            uid, old_version
                        ),
                    });
                }

                Ok(())
            })
        })
        .await?;

        let _ = self.cache.delete(&Self::cache_key(metadata.account_id())).await;
        Ok(())
    }
}
