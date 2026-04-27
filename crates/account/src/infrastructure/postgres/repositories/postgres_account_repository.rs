// crates/account/src/infrastructure/postgres/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use sqlx::{Pool, Postgres, query_scalar};
use std::sync::Arc;

use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;

use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{Email, ExternalId, PhoneNumber};
use crate::infrastructure::postgres::rows::{
    PostgresAccountGovernanceRow, PostgresAccountIdentityRow, PostgresAccountRow,
    PostgresAccountSettingsRow,
};

pub struct PostgresAccountRepository {
    pool: Pool<Postgres>,
    cache: Arc<dyn CacheRepository>,
}

impl PostgresAccountRepository {
    pub fn new(pool: Pool<Postgres>, cache: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache }
    }

    fn cache_key(id: &AccountId) -> String {
        format!("account:aggregate:{}", id.as_uuid())
    }
}

#[async_trait]
impl AccountRepository for PostgresAccountRepository {
    /// Récupère l'agrégat complet (Identity + Governance + Settings)
    async fn find_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let key = Self::cache_key(id);

        // 1. Stratégie de Cache (uniquement hors transaction)
        let is_no_tx = tx.is_none();

        if is_no_tx {
            if let Ok(Some(account)) = self.cache.get_obj::<Account>(&key).await {
                return Ok(Some(account));
            }
        }

        let uid = id.as_uuid();

        // 2. Récupération des données via une seule requête JOIN
        let account_opt = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
                SELECT 
                    i.*, 
                    g.role, g.is_beta_tester, g.is_shadowbanned, g.trust_score, 
                    g.moderation_notes, g.last_moderation_at, g.last_ip_addr,
                    s.preferences, s.timezone, s.push_tokens
                FROM account_identity i
                JOIN account_governance g ON i.account_id = g.account_id
                JOIN account_settings s ON i.account_id = s.account_id
                WHERE i.account_id = $1
            "#;

                let row_opt = sqlx::query_as::<_, PostgresAccountRow>(sql)
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("Account: fetch aggregate join")?;

                // On utilise le to_domain() que nous venons d'implémenter
                match row_opt {
                    Some(row) => Ok(Some(row.to_domain()?)),
                    None => Ok(None),
                }
            })
        })
        .await?;

        // 3. Mise à jour du Cache
        if is_no_tx {
            if let Some(account) = &account_opt {
                let _ = self
                    .cache
                    .set_obj(&key, account, Some(std::time::Duration::from_secs(900)))
                    .await;
            }
        }

        Ok(account_opt)
    }

    /// Sauvegarde atomique avec gestion de la concurrence (OCC)
    /// ( !!! Not optimized for partial updates, Todo later)
    async fn save(&self, account: &mut Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let ident_row = PostgresAccountIdentityRow::from_domain(account);
        let gov_row = PostgresAccountGovernanceRow::from_domain(account);
        let sett_row = PostgresAccountSettingsRow::from_domain(account);

        // Données pour l'OCC (Optimistic Concurrency Control)
        let uid = ident_row.account_id;
        let current_version = account.metadata().version() as i64;
        let next_version = ident_row.version;

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
        Box::pin(async move {
            // --- 1. UPDATE IDENTITY (avec vérification de version) ---
            let sql_identity = r#"
                UPDATE account_identity SET 
                    email = $2, email_verified = $3, phone_number = $4, phone_verified = $5,
                    state = $6, locale = $7, version = $8, aggregate_updated_at = $9, last_active_at = $10
                WHERE account_id = $1 AND version = $11"#;

            let res = sqlx::query(sql_identity)
                .bind(uid)
                .bind(ident_row.email)
                .bind(ident_row.email_verified)
                .bind(ident_row.phone_number)
                .bind(ident_row.phone_verified)
                .bind(ident_row.state)
                .bind(ident_row.locale)
                .bind(next_version)
                .bind(ident_row.aggregate_updated_at)
                .bind(ident_row.last_active_at)
                .bind(current_version)
                .execute(&mut *conn)
                .await
                .map_domain_infra("Account: update identity")?;

            // Si aucune ligne n'est modifiée, c'est qu'un autre thread a modifié l'agrégat
            if res.rows_affected() == 0 {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!("Account {}: OCC mismatch (expected v{})", uid, current_version),
                });
            }

            // --- 2. UPDATE GOVERNANCE ---
            let sql_gov = r#"
                UPDATE account_governance SET
                    role = $2, is_beta_tester = $3, is_shadowbanned = $4,
                    trust_score = $5, moderation_notes = $6, last_moderation_at = $7, last_ip_addr = $8
                WHERE account_id = $1"#;

            sqlx::query(sql_gov)
                .bind(uid)
                .bind(gov_row.role)
                .bind(gov_row.is_beta_tester)
                .bind(gov_row.is_shadowbanned)
                .bind(gov_row.trust_score)
                .bind(gov_row.moderation_notes)
                .bind(gov_row.last_moderation_at)
                .bind(gov_row.last_ip_addr)
                .execute(&mut *conn)
                .await
                .map_domain_infra("Account: update governance")?;

            // --- 3. UPDATE SETTINGS ---
            let sql_settings = r#"
                UPDATE account_settings SET
                    preferences = $2, timezone = $3, push_tokens = $4
                WHERE account_id = $1"#;

            sqlx::query(sql_settings)
                .bind(uid)
                .bind(sett_row.preferences)
                .bind(sett_row.timezone)
                .bind(sett_row.push_tokens)
                .execute(&mut *conn)
                .await
                .map_domain_infra("Account: update settings")?;

            Ok(())
        })
    }).await?;

        // --- 4. POST-TRANSACTION ---
        // On met à jour les métadonnées de l'objet en mémoire après le succès DB
        account.metadata_mut().record_change();

        // Invalidation du cache
        let _ = self
            .cache
            .delete(&Self::cache_key(account.identity().account_id()))
            .await;

        Ok(())
    }

    async fn find_id_by_email(&self, email: &Email) -> Result<Option<AccountId>> {
        let sql = "SELECT account_id FROM account_identity WHERE email = $1";
        let res = query_scalar::<_, uuid::Uuid>(sql)
            .bind(email.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain_infra("Account: find_id_by_email")?;

        Ok(res.map(AccountId::from_uuid))
    }

    async fn find_id_by_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        let sql = "SELECT account_id FROM account_identity WHERE external_id = $1";
        let res = query_scalar::<_, uuid::Uuid>(sql)
            .bind(ext_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain_infra("Account: find_id_by_external_id")?;

        Ok(res.map(AccountId::from_uuid))
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE email = $1)";
        let exists = query_scalar::<_, bool>(sql)
            .bind(email.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain_infra("Account: exists_by_email")?;
        Ok(exists)
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE phone_number = $1)";
        let exists = query_scalar::<_, bool>(sql)
            .bind(phone.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain_infra("Account: exists_by_phone")?;
        Ok(exists)
    }

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE external_id = $1)";
        let exists = query_scalar::<_, bool>(sql)
            .bind(ext_id.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain_infra("Account: exists_by_external_id")?;
        Ok(exists)
    }

    async fn create(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()> {
        // 1. Préparation des DTOs (Rows)
        let ident_row = PostgresAccountIdentityRow::from_domain(account);
        let gov_row = PostgresAccountGovernanceRow::from_domain(account);
        let sett_row = PostgresAccountSettingsRow::from_domain(account);

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
        Box::pin(async move {
            // --- 1. INSERT IDENTITY ---
            sqlx::query(
                r#"INSERT INTO account_identity 
                    (account_id, external_id, email, email_verified, state, locale, version, aggregate_updated_at, last_active_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
            )
            .bind(ident_row.account_id)
            .bind(ident_row.external_id)
            .bind(ident_row.email)
            .bind(ident_row.email_verified)
            .bind(ident_row.state)
            .bind(ident_row.locale)
            .bind(ident_row.version)
            .bind(ident_row.aggregate_updated_at)
            .bind(ident_row.last_active_at)
            .execute(&mut *conn)
            .await
            .map_domain_infra("Account: insert identity")?;

            // --- 2. INSERT GOVERNANCE ---
            sqlx::query(
                r#"INSERT INTO account_governance 
                    (account_id, role, is_beta_tester, is_shadowbanned, trust_score, moderation_notes, last_moderation_at, last_ip_addr)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#
            )
            .bind(gov_row.account_id)
            .bind(gov_row.role)
            .bind(gov_row.is_beta_tester)
            .bind(gov_row.is_shadowbanned)
            .bind(gov_row.trust_score)
            .bind(gov_row.moderation_notes)
            .bind(gov_row.last_moderation_at)
            .bind(gov_row.last_ip_addr)
            .execute(&mut *conn)
            .await
            .map_domain_infra("Account: insert governance")?;

            // --- 3. INSERT SETTINGS ---
            sqlx::query(
                r#"INSERT INTO account_settings 
                    (account_id, preferences, timezone, push_tokens)
                   VALUES ($1, $2, $3, $4)"#
            )
            .bind(sett_row.account_id)
            .bind(sett_row.preferences)
            .bind(sett_row.timezone)
            .bind(sett_row.push_tokens)
            .execute(&mut *conn)
            .await
            .map_domain_infra("Account: insert settings")?;

            Ok(())
        })
    })
    .await
    }

    async fn delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                let sql = "DELETE FROM account_identity WHERE account_id = $1";

                sqlx::query(sql)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain_infra("Account: delete")?;

                Ok(())
            })
        })
        .await?;

        // Invalidation du cache APRÈS la réussite de la transaction
        // Note : Idéalement, l'invalidation du cache se fait après le commit final,
        // mais ici on suit ta logique habituelle de repository.
        let _ = self.cache.delete(&Self::cache_key(id)).await;

        Ok(())
    }
}
