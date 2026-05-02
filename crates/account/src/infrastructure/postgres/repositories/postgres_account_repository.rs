// crates/account/src/infrastructure/postgres/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use sqlx::{Pool, Postgres, query_scalar};
use std::sync::Arc;

use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};
use shared_kernel::domain::value_objects::{AccountId, Email, PhoneNumber, SubId};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;

use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;
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

    pub fn cache_key(id: &AccountId) -> String {
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

        // 1. Stratégie de Cache (uniquement hors transaction)
        if tx.is_none() {
            // --- DIAGNOSTIC RADICAL ---
            // On utilise spawn_blocking ou un timeout très court pour être SÛR
            // que si le cache est mort, on ne bloque pas le thread principal.
            let cache_handle = self.cache.clone();
            let cache_key = key.clone();

            let cache_result = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                self.cache.get_obj::<Account>(&cache_key),
            )
            .await;

            match cache_result {
                Ok(Ok(Some(account))) => return Ok(Some(account)),
                Ok(Err(e)) => {
                    tracing::error!(
                        error = %e,
                        account_id = %id,
                        "Cache retrieval failed"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        timeout_ms = 50,
                        account_id = %id,
                        "Cache timeout - potential deadlock avoided, falling back to DB"
                    );
                }
                _ => {}
            }
        }

        let uid = id.as_uuid();

        // 2. Récupération des données via une seule requête JOIN
        let account_opt = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
    SELECT 
        i.*, 
        i.updated_at as identity_updated_at,
        s.preferences, s.timezone, s.push_tokens, 
        s.updated_at as settings_updated_at,
        g.role, g.is_beta_tester, g.is_shadowbanned, g.trust_score, g.moderation_notes, 
        g.last_moderation_at, g.last_ip_addr, 
        g.updated_at as governance_updated_at
    FROM account_identity i
    LEFT JOIN account_settings s ON i.account_id = s.account_id
    LEFT JOIN account_governance g ON i.account_id = g.account_id
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
                let cache_handle = self.cache.clone();
                let cache_key = key.clone();
                let account_to_cache = account.clone();

                // On ne bloque PAS la sortie de la fonction pour le cache
                let _ = tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    cache_handle.set_obj(
                        &cache_key,
                        &account_to_cache,
                        Some(std::time::Duration::from_secs(900)),
                    ),
                )
                .await;
            }
        }

        Ok(account_opt)
    }

    async fn find_by_sub_id(
        &self,
        ext_id: &SubId,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let account_id_opt = self.find_id_by_sub_id(ext_id, tx.as_deref_mut()).await?;

        match account_id_opt {
            Some(id) => self.find_by_id(&id, tx).await,
            None => Ok(None),
        }
    }

    /// Sauvegarde atomique avec gestion de la concurrence (OCC)
    /// ( !!! Not optimized for partial updates, Todo later)
    async fn save(&self, account: &mut Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let ident_row = PostgresAccountIdentityRow::from_domain(account);
        let gov_row = PostgresAccountGovernanceRow::from_domain(account);
        let sett_row = PostgresAccountSettingsRow::from_domain(account);

        // Données pour l'OCC (Optimistic Concurrency Control)
        let uid = ident_row.account_id;
        let next_version = account.metadata().version() as i64;
        let current_version = next_version - 1;

        // Dans account_repository.rs, méthode save ou create
        let current_v = account.metadata().version();
        println!(
            "DEBUG: Tentative de mise à jour. Version dans l'objet Rust: {}",
            current_v
        );

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
        Box::pin(async move {
            // --- 1. UPDATE IDENTITY (avec vérification de version) ---
            let sql_identity = r#"
                UPDATE account_identity SET 
                    sub_id = $2,
                    email = $3, phone_number = $4,
                    state = $5, locale = $6, version = $7, last_active_at = $8
                WHERE account_id = $1 AND version = $9"#;

            let res = sqlx::query(sql_identity)
                .bind(uid)
                .bind(ident_row.sub_id)
                .bind(ident_row.email)
                .bind(ident_row.phone_number)
                .bind(ident_row.state)
                .bind(ident_row.locale)
                .bind(next_version)
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
        account.metadata_mut().record_change();

        let cache_handle = self.cache.clone();
        let key = Self::cache_key(account.identity().account_id());

        // On ne fait plus de .await ici, on lance la tâche en fond
        tokio::spawn(async move {
            let _ = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                cache_handle.delete(&key),
            )
            .await;
        });

        Ok(())
    }

    async fn find_id_by_email(
        &self,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        let email_raw = email.to_string();
        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let email_owned = email_raw.clone();
            Box::pin(async move {
                let sql = "SELECT account_id FROM account_identity WHERE email = $1";
                let res = query_scalar::<_, uuid::Uuid>(sql)
                    .bind(email_owned)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("Account: find_id_by_email")?;
                Ok(res.map(AccountId::from_uuid))
            })
        })
        .await
    }
    async fn find_id_by_sub_id(
        &self,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        let ext_id_raw = ext_id.to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let ext_id_owned = ext_id_raw.clone();

            Box::pin(async move {
                let sql = "SELECT account_id FROM account_identity WHERE sub_id = $1";
                let res = sqlx::query_scalar::<_, uuid::Uuid>(sql)
                    .bind(ext_id_owned)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("Account: find_id_by_sub_id")?;

                Ok(res.map(AccountId::from_uuid))
            })
        })
        .await
    }

    async fn exists_by_email(
        &self,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let email_raw = email.to_string();
        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let email_owned = email_raw.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE email = $1)";
                let exists = query_scalar::<_, bool>(sql)
                    .bind(email_owned)
                    .fetch_one(conn)
                    .await
                    .map_domain_infra("Account: exists_by_email")?;
                Ok(exists)
            })
        })
        .await
    }

    async fn exists_by_phone(
        &self,
        phone: &PhoneNumber,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let phone_raw = phone.to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let phone_owned = phone_raw.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE phone_number = $1)";
                let exists = sqlx::query_scalar::<_, bool>(sql)
                    .bind(phone_owned)
                    .fetch_one(conn)
                    .await
                    .map_domain_infra("Account: exists_by_phone")?;
                Ok(exists)
            })
        })
        .await
    }

    async fn exists_by_sub_id(
        &self,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let ext_id_raw = ext_id.to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            let ext_id_owned = ext_id_raw.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE sub_id = $1)";
                let exists = sqlx::query_scalar::<_, bool>(sql)
                    .bind(ext_id_owned)
                    .fetch_one(conn) // 3. Utilisation de conn
                    .await
                    .map_domain_infra("Account: exists_by_sub_id")?;
                Ok(exists)
            })
        })
        .await
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
                    (account_id, sub_id, email, state, locale, version, last_active_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#
            )
            .bind(ident_row.account_id)
            .bind(ident_row.sub_id)
            .bind(ident_row.email)
            .bind(ident_row.state)
            .bind(ident_row.locale)
            .bind(ident_row.version)
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
        let cache_handle = self.cache.clone();
        let key = Self::cache_key(id);

        tokio::spawn(async move {
            let _ = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                cache_handle.delete(&key),
            )
            .await;
        });

        Ok(())
    }
}
