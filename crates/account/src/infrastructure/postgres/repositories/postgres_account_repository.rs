// crates/account/src/infrastructure/postgres/repositories/account_repository.rs

use async_trait::async_trait;
use sqlx::{Pool, Postgres, query_scalar};

use shared_kernel::{core::{AggregateRoot, Error, Identifier, Result, Transaction}, types::{AccountId, Email, PhoneNumber, SubId}};

use crate::domain::entities::Account;
use crate::domain::repositories::AccountRepository;
use crate::infrastructure::postgres::rows::{
    PostgresAccountGovernanceRow, PostgresAccountIdentityRow, PostgresAccountRow,
    PostgresAccountSettingsRow,
};

pub struct PostgresAccountRepository {
    pool: Pool<Postgres>,
}

impl PostgresAccountRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountRepository for PostgresAccountRepository {
    /// Récupère l'agrégat complet (Identity + Governance + Settings)
    async fn find_by_id(
        &self,
        id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {

        // 2. Fallback DB (Si pas en cache ou si transaction active)
        let uid = id.uuid();
        let region = id.region();

        let account_opt = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
                    SELECT i.*, i.updated_at as identity_updated_at,
                           s.preferences, s.timezone, s.push_tokens, s.updated_at as settings_updated_at,
                           g.role, g.beta_tier, g.is_shadowbanned, g.trust_score, g.moderation_notes, 
                           g.last_moderation_at, g.last_ip_addr, g.updated_at as governance_updated_at
                    FROM account_identity i
                    LEFT JOIN account_settings s ON i.account_id = s.account_id AND i.region = s.region
                    LEFT JOIN account_governance g ON i.account_id = g.account_id AND i.region = g.region
                    WHERE i.account_id = $1 AND i.region = $2"#;

                let row_opt = sqlx::query_as::<_, PostgresAccountRow>(sql)
                    .bind(uid)
                    .bind(region.as_str())
                    .fetch_optional(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account find_by_id repository: {}", e.to_string())))?;

                match row_opt {
                    Some(row) => Ok(Some(row.to_domain()?)),
                    None => Ok(None),
                }
            })
        }).await?;

        Ok(account_opt)
    }

    async fn find_by_sub_id(
        &self,
        ext_id: &SubId,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let account_id_opt = self.find_id_by_sub_id(ext_id, tx.as_deref_mut()).await?;

        match account_id_opt {
            Some(id) => self.find_by_id(id, tx).await,
            None => Ok(None),
        }
    }

    /// Sauvegarde atomique avec gestion de la concurrence (OCC)
    /// ( !!! Not optimized for partial updates, Todo later)
    async fn save(&self, account: &mut Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let ident_row = PostgresAccountIdentityRow::from_domain(account);
        let gov_row = PostgresAccountGovernanceRow::from_domain(account);
        let sett_row = PostgresAccountSettingsRow::from_domain(account);

        let next_version = account.metadata().version() as i64;
        let is_events_empty = account.metadata().is_events_empty();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                // 1. On vérifie si le compte existe et on verrouille la ligne (FOR UPDATE) 
                // pour garantir que personne ne modifie la version entre le check et l'écriture
                let db_v: Option<i64> = sqlx::query_scalar(
                    "SELECT version FROM account_identity WHERE account_id = $1 AND region = $2 FOR UPDATE"
                )
                .bind(&ident_row.account_id)
                .bind(&ident_row.region)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;

                match db_v {
                    None => {
                        // --- MODE INSERT ---
                        // On insère l'identité
                        sqlx::query(
                            r#"INSERT INTO account_identity 
                                (account_id, region, sub_id, email, state, locale, version, last_active_at)
                               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#
                        )
                        .bind(&ident_row.account_id).bind(&ident_row.region).bind(ident_row.sub_id).bind(ident_row.email)
                        .bind(ident_row.state).bind(ident_row.locale)
                        .bind(next_version).bind(ident_row.last_active_at)
                        .execute(&mut *conn).await.map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;

                        // On insère la gouvernance
                        sqlx::query(
                            r#"INSERT INTO account_governance 
                                (account_id, region, role, beta_tier, is_shadowbanned, trust_score, moderation_notes, last_moderation_at, last_ip_addr)
                               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
                        )
                        .bind(&ident_row.account_id).bind(&ident_row.region).bind(gov_row.role).bind(gov_row.beta_tier)
                        .bind(gov_row.is_shadowbanned).bind(gov_row.trust_score)
                        .bind(gov_row.moderation_notes).bind(gov_row.last_moderation_at)
                        .bind(gov_row.last_ip_addr)
                        .execute(&mut *conn).await.map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;

                        // On insère les settings
                        sqlx::query(
                            r#"INSERT INTO account_settings (account_id, region, preferences, timezone, push_tokens)
                               VALUES ($1, $2, $3, $4, $5)"#
                        )
                        .bind(&ident_row.account_id).bind(&ident_row.region).bind(sett_row.preferences).bind(sett_row.timezone).bind(sett_row.push_tokens)
                        .execute(&mut *conn).await.map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;
                    }
                    Some(v) => {
                        // --- MODE UPDATE (OCC) ---
                        // Si l'application soumet la même version que la DB,
                        // c'est une écriture technique d'idempotence. La version cible reste `v`.
                        let is_noop = next_version == v && is_events_empty;
                        
                        let target_version = if is_noop { v } else { next_version };
                        let current_version_expected = target_version - 1;

                        if !is_noop && v != current_version_expected {
                            return Err(Error::concurrency_conflict(
                                format!("Account {}: OCC mismatch (DB v{}, App expected v{})", &ident_row.account_id, v, current_version_expected),
                            ));
                        }

                        // Update Identity
                        sqlx::query(
                                r#"UPDATE account_identity SET 
                                    sub_id = $3, email = $4, state = $5, locale = $6, version = $7, last_active_at = $8
                                WHERE account_id = $1 AND region = $2"#
                            )
                        .bind(&ident_row.account_id)
                        .bind(&ident_row.region)
                        .bind(ident_row.sub_id)
                        .bind(ident_row.email)
                        .bind(ident_row.state)
                        .bind(ident_row.locale)
                        .bind(target_version)
                        .bind(ident_row.last_active_at)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;

                        // Update Governance
                        sqlx::query(
                            r#"UPDATE account_governance SET
                                role = $3, beta_tier = $4, is_shadowbanned = $5, trust_score = $6, 
                                moderation_notes = $7, last_moderation_at = $8, last_ip_addr = $9
                            WHERE account_id = $1 AND region = $2"#
                        )
                        .bind(&ident_row.account_id)
                        .bind(&ident_row.region)
                        .bind(gov_row.role)
                        .bind(gov_row.beta_tier)
                        .bind(gov_row.is_shadowbanned)
                        .bind(gov_row.trust_score)
                        .bind(gov_row.moderation_notes)
                        .bind(gov_row.last_moderation_at)
                        .bind(gov_row.last_ip_addr)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;


                        // Update Settings
                        sqlx::query(
                            r#"UPDATE account_settings SET 
                                preferences = $3, timezone = $4, push_tokens = $5
                            WHERE account_id = $1 AND region = $2"#
                        )
                        .bind(&ident_row.account_id)
                        .bind(&ident_row.region)
                        .bind(sett_row.preferences)
                        .bind(sett_row.timezone) 
                        .bind(sett_row.push_tokens)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account save repository: {}", e.to_string())))?;
                    }
                }
                Ok(())
            })
        }).await?;

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
                    .map_err(|e| Error::database(format!("Account find_id_by_email repository: {}", e.to_string())))?;
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
                    .map_err(|e| Error::database(format!("Account find_id_by_sub_id repository: {}", e.to_string())))?;

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
                    .map_err(|e| Error::database(format!("Account exists_by_email repository: {}", e.to_string())))?;
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
                    .map_err(|e| Error::database(format!("Account exists_by_phone repository: {}", e.to_string())))?;
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
                    .map_err(|e| Error::database(format!("Account exists_by_sub_id repository: {}", e.to_string())))?;
                Ok(exists)
            })
        })
        .await
    }

    async fn create(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()> {
        let mut acc = account.clone();
        self.save(&mut acc, Some(tx)).await
    }

    async fn delete(&self, id: AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                let sql = "DELETE FROM account_identity WHERE account_id = $1";

                sqlx::query(sql)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account delete repository: {}", e.to_string())))?;

                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}
