use async_trait::async_trait;
use infra_sqlx::sqlx::{self, Pool, Postgres, query_scalar};
use infra_sqlx::TransactionExecuteExt;

use shared_kernel::core::{AggregateRoot, Entity, Identifier};
use shared_kernel::{
    core::{Error, Result, Transaction},
    types::{AccountId, Email, PhoneNumber, Region, SubId},
};

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
    async fn find_by_id(
        &self,
        region: Region,
        id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>
    {
        let uid = id.uuid();
        let region_str = region.to_string();

        let account_opt = self.pool.execute_on(tx, |conn| {
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
                    .bind(region_str)
                    .fetch_optional(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account find_by_id repository: {}", e)))?;

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
        region: Region,
        ext_id: &SubId,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let account_id_opt = self.find_id_by_sub_id(region, ext_id, tx.as_deref_mut()).await?;

        match account_id_opt {
            Some(id) => self.find_by_id(region, id, tx).await,
            None => Ok(None),
        }
    }

    async fn save(
        &self, 
        region: Region, 
        account: &mut Account, 
        tx: Option<&mut dyn Transaction>
    ) -> Result<()> {
        let region_str = region.to_string();
        let ident_row = PostgresAccountIdentityRow::from_domain(account);
        let gov_row = PostgresAccountGovernanceRow::from_domain(account);
        let sett_row = PostgresAccountSettingsRow::from_domain(account);

        let next_version = account.metadata().version() as i64;
        let is_events_empty = account.metadata().is_events_empty();
        let agg_created_at = account.metadata().created_at();
        let agg_updated_at = account.metadata().updated_at();
        let ident_updated_at = account.identity().updated_at();
        let gov_updated_at = account.governance().updated_at();
        let sett_updated_at = account.settings().updated_at();


        self.pool.execute_on(tx, |conn| {
            Box::pin(async move {
                // 1. Lock d'idempotence et d'OCC (FOR UPDATE) aligné territorialement
                let db_v: Option<i64> = sqlx::query_scalar(
                    "SELECT version FROM account_identity WHERE account_id = $1 AND region = $2 FOR UPDATE"
                )
                .bind(&ident_row.account_id)
                .bind(&region_str)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| Error::database(format!("Account save repository lookup: {}", e)))?;

                match db_v {
                    None => {
                        sqlx::query(
                            r#"INSERT INTO account_identity 
                                (
                                    account_id, region, sub_id, email, phone_number, 
                                    state, birth_date, locale, version, last_active_at,
                                    created_at, updated_at, aggregate_updated_at
                                )
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#
                        )
                        .bind(&ident_row.account_id)
                        .bind(&region_str)
                        .bind(&ident_row.sub_id)
                        .bind(&ident_row.email)
                        .bind(&ident_row.phone_number)
                        .bind(&ident_row.state)
                        .bind(&ident_row.birth_date)
                        .bind(&ident_row.locale)
                        .bind(next_version)
                        .bind(ident_row.last_active_at)
                        .bind(agg_created_at)
                        .bind(ident_updated_at)
                        .bind(agg_updated_at)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account insert identity failed: {}", e)))?;

                        sqlx::query(
                            r#"INSERT INTO account_governance 
                                (account_id, region, role, beta_tier, is_shadowbanned, trust_score, moderation_notes, last_moderation_at, last_ip_addr, updated_at)
                               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#
                        )
                        .bind(&ident_row.account_id).bind(&region_str).bind(gov_row.role).bind(gov_row.beta_tier)
                        .bind(gov_row.is_shadowbanned).bind(gov_row.trust_score)
                        .bind(gov_row.moderation_notes).bind(gov_row.last_moderation_at)
                        .bind(gov_row.last_ip_addr).bind(gov_updated_at)
                        .execute(&mut *conn).await.map_err(|e| Error::database(format!("Account insert governance: {}", e)))?;

                        sqlx::query(
                            r#"INSERT INTO account_settings (account_id, region, preferences, timezone, push_tokens, updated_at)
                               VALUES ($1, $2, $3, $4, $5, $6)"#
                        )
                        .bind(&ident_row.account_id).bind(&region_str).bind(sett_row.preferences).bind(sett_row.timezone).bind(sett_row.push_tokens).bind(sett_updated_at)
                        .execute(&mut *conn).await.map_err(|e| Error::database(format!("Account insert settings: {}", e)))?;
                    }
                    Some(v) => {
                        // --- MODE UPDATE (OCC) ---
                        let is_noop = next_version == v && is_events_empty;
                        let target_version = if is_noop { v } else { next_version };
                        let current_version_expected = target_version - 1;

                        if !is_noop && v != current_version_expected {
                            return Err(Error::concurrency_conflict(
                                format!("Account {}: OCC mismatch (DB v{}, App expected v{})", &ident_row.account_id, v, current_version_expected),
                            ));
                        }

                       sqlx::query(
                                r#"UPDATE account_identity SET 
                                    sub_id = $3, 
                                    email = $4, 
                                    phone_number = $5, 
                                    state = $6, 
                                    birth_date = $7, 
                                    locale = $8, 
                                    version = $9, 
                                    last_active_at = $10,
                                    updated_at = $11,
                                    aggregate_updated_at = $12
                                WHERE account_id = $1 AND region = $2"#
                            )
                        .bind(&ident_row.account_id)
                        .bind(&region_str)
                        .bind(&ident_row.sub_id)
                        .bind(&ident_row.email)
                        .bind(&ident_row.phone_number)
                        .bind(&ident_row.state)
                        .bind(&ident_row.birth_date)
                        .bind(&ident_row.locale)
                        .bind(target_version)
                        .bind(ident_row.last_active_at)
                        .bind(ident_updated_at)
                        .bind(agg_updated_at)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account update identity failed: {}", e)))?;

                        sqlx::query(
                            r#"UPDATE account_governance SET
                                role = $3, beta_tier = $4, is_shadowbanned = $5, trust_score = $6, 
                                moderation_notes = $7, last_moderation_at = $8, last_ip_addr = $9, updated_at = $10
                            WHERE account_id = $1 AND region = $2"#
                        )
                        .bind(&ident_row.account_id)
                        .bind(&region_str)
                        .bind(gov_row.role)
                        .bind(gov_row.beta_tier)
                        .bind(gov_row.is_shadowbanned)
                        .bind(gov_row.trust_score)
                        .bind(gov_row.moderation_notes)
                        .bind(gov_row.last_moderation_at)
                        .bind(gov_row.last_ip_addr)
                        .bind(gov_updated_at)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account update governance: {}", e)))?;

                        sqlx::query(
                            r#"UPDATE account_settings SET 
                                preferences = $3, timezone = $4, push_tokens = $5, updated_at = $6
                            WHERE account_id = $1 AND region = $2"#
                        )
                        .bind(&ident_row.account_id)
                        .bind(&region_str)
                        .bind(sett_row.preferences)
                        .bind(sett_row.timezone) 
                        .bind(sett_row.push_tokens)
                        .bind(sett_updated_at)
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| Error::database(format!("Account update settings: {}", e)))?;
                    }
                }
                Ok(())
            })
        }).await?;

        Ok(())
    }

    async fn find_id_by_email(
        &self,
        region: Region,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        let email_raw = email.to_string();
        let region_str = region.to_string();

        self.pool.execute_on(tx, |conn| {
            let email_owned = email_raw.clone();
            let region_owned = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT account_id FROM account_identity WHERE email = $1 AND region = $2";
                let res = query_scalar::<_, uuid::Uuid>(sql)
                    .bind(email_owned)
                    .bind(region_owned)
                    .fetch_optional(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account find_id_by_email: {}", e)))?;
                Ok(res.map(AccountId::from_uuid))
            })
        })
        .await
    }

    async fn find_id_by_sub_id(
        &self,
        region: Region,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        let ext_id_raw = ext_id.to_string();
        let region_str = region.to_string();

        self.pool.execute_on(tx, |conn| {
            let ext_id_owned = ext_id_raw.clone();
            let region_owned = region_str.clone();

            Box::pin(async move {
                let sql = "SELECT account_id FROM account_identity WHERE sub_id = $1 AND region = $2";
                let res = sqlx::query_scalar::<_, uuid::Uuid>(sql)
                    .bind(ext_id_owned)
                    .bind(region_owned)
                    .fetch_optional(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account find_id_by_sub_id: {}", e)))?;

                Ok(res.map(AccountId::from_uuid))
            })
        })
        .await
    }

    async fn exists_by_email(
        &self,
        region: Region,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let email_raw = email.to_string();
        let region_str = region.to_string();

        self.pool.execute_on(tx, |conn| {
            let email_owned = email_raw.clone();
            let region_owned = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE email = $1 AND region = $2)";
                let exists = query_scalar::<_, bool>(sql)
                    .bind(email_owned)
                    .bind(region_owned)
                    .fetch_one(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account exists_by_email: {}", e)))?;
                Ok(exists)
            })
        })
        .await
    }

    async fn exists_by_phone(
        &self,
        region: Region,
        phone: &PhoneNumber,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let phone_raw = phone.to_string();
        let region_str = region.to_string();

        self.pool.execute_on(tx, |conn| {
            let phone_owned = phone_raw.clone();
            let region_owned = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE phone_number = $1 AND region = $2)";
                let exists = sqlx::query_scalar::<_, bool>(sql)
                    .bind(phone_owned)
                    .bind(region_owned)
                    .fetch_one(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account exists_by_phone: {}", e)))?;
                Ok(exists)
            })
        })
        .await
    }

    async fn exists_by_sub_id(
        &self,
        region: Region,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        let ext_id_raw = ext_id.to_string();
        let region_str = region.to_string();

        self.pool.execute_on(tx, |conn| {
            let ext_id_owned = ext_id_raw.clone();
            let region_owned = region_str.clone();
            Box::pin(async move {
                let sql = "SELECT EXISTS(SELECT 1 FROM account_identity WHERE sub_id = $1 AND region = $2)";
                let exists = sqlx::query_scalar::<_, bool>(sql)
                    .bind(ext_id_owned)
                    .bind(region_owned)
                    .fetch_one(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account exists_by_sub_id: {}", e)))?;
                Ok(exists)
            })
        })
        .await
    }

    async fn create(&self, region: Region, account: &Account, tx: &mut dyn Transaction) -> Result<()> {
        let mut acc = account.clone();
        self.save(region, &mut acc, Some(tx)).await
    }

    async fn delete(&self, region: Region, id: AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.uuid();
        let region_str = region.to_string();

        self.pool.execute_on(Some(tx), |conn| {
            Box::pin(async move {
                let sql = "DELETE FROM account_identity WHERE account_id = $1 AND region = $2";

                sqlx::query(sql)
                    .bind(uid)
                    .bind(region_str)
                    .execute(conn)
                    .await
                    .map_err(|e| Error::database(format!("Account delete repository: {}", e)))?;

                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}