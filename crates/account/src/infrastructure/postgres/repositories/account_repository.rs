// crates/account/src/infrastructure/postgres/repositories/account_repository

use async_trait::async_trait;
use sqlx::{Postgres, Pool, query, query_as, query_scalar, QueryBuilder};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Username};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::SqlxErrorExt;

use crate::domain::entities::Account;
use crate::domain::params::PatchUserParams;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{Email, PhoneNumber, ExternalId, AccountState};
use crate::infrastructure::postgres::rows::PostgresAccountRow;

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

    // --- RECHERCHES ---

    async fn find_account_id_by_email(&self, email: &Email) -> Result<Option<AccountId>> {
        let email_str = email.as_str().to_string();

        let id = <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM users WHERE email = $1")
                .bind(email_str)
                .fetch_optional(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        Ok(id.map(AccountId::new_unchecked))
    }

    async fn find_account_id_by_username(&self, username: &Username) -> Result<Option<AccountId>> {
        let username_str = username.as_str().to_string();

        let id = <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM users WHERE username = $1")
                .bind(username_str)
                .fetch_optional(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        Ok(id.map(AccountId::new_unchecked))
    }

    async fn find_account_id_by_external_id(
        &self,
        external_id: &ExternalId,
        tx: Option<&mut dyn Transaction>
    ) -> Result<Option<AccountId>> {
        let ext_id = external_id.as_str().to_string();

        // 2. On passe 'tx' au lieu de 'None' dans execute_on
        let id = <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM users WHERE external_id = $1")
                .bind(ext_id)
                .fetch_optional(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        Ok(id.map(AccountId::new_unchecked))
    }

    async fn find_account_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>
    ) -> Result<Option<Account>> {
        let uid = id.as_uuid();
        let has_tx = tx.is_some();

        let row = <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            let sql = if has_tx {
                "SELECT * FROM users WHERE id = $1 FOR UPDATE"
            } else {
                "SELECT * FROM users WHERE id = $1"
            };

            query_as::<_, PostgresAccountRow>(sql)
                .bind(uid)
                .fetch_optional(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        row.map(Account::try_from).transpose()
    }

    // --- VÉRIFICATIONS ---

    async fn exists_account_by_email(&self, email: &Email) -> Result<bool> {
        let email_str = email.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query_scalar::<Postgres, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
                .bind(email_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
        }))
            .await
    }

    async fn exists_account_by_username(&self, username: &Username) -> Result<bool> {
        let username_str = username.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query_scalar::<Postgres, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
                .bind(username_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
        }))
            .await
    }

    async fn exists_account_by_phone_number(&self, phone: &PhoneNumber) -> Result<bool> {
        let phone_str = phone.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query_scalar::<Postgres, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE phone_number = $1)")
                .bind(phone_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
        }))
            .await
    }

    // --- ÉCRITURES ---

    async fn create_account(&self, user: &Account, tx: &mut dyn Transaction) -> Result<()> {
        let uid = user.id.as_uuid();
        let region = user.region_code.as_str().to_string();
        let ext_id = user.external_id.as_str().to_string();
        let username = user.username.as_str().to_string();
        let email = user.email.as_str().to_string();
        let phone = user.phone_number.as_ref().map(|p| p.as_str().to_string());
        let state = user.account_state.as_str().to_string();
        let birth = user.birth_date.as_ref().map(|d| d.value());
        let locale = user.locale.as_str().to_string();
        let created = user.created_at;
        let updated = user.updated_at;
        let active = user.last_active_at;

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| Box::pin(async move {
            query(
                r#"
                INSERT INTO users (
                    id, region_code, external_id, username, email,
                    phone_number, account_state, birth_date, locale,
                    created_at, updated_at, last_active_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#
            )
                .bind(uid).bind(region).bind(ext_id).bind(username).bind(email)
                .bind(phone).bind(state).bind(birth).bind(locale)
                .bind(created).bind(updated).bind(active)
                .execute(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        Ok(())
    }

    async fn patch_account_by_id(&self, id: &AccountId, params: PatchUserParams, tx: &mut dyn Transaction) -> Result<()> {
        if params.is_empty() { return Ok(()); }
        let uid = id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| Box::pin(async move {
            let mut qb = QueryBuilder::<Postgres>::new("UPDATE users SET ");
            let mut separated = qb.separated(", ");

            if let Some(u) = params.username { separated.push("username = ").push_bind(u.as_str().to_string()); }
            if let Some(e) = params.email { separated.push("email = ").push_bind(e.as_str().to_string()); }
            if let Some(v) = params.email_verified { separated.push("email_verified = ").push_bind(v); }
            if let Some(p) = params.phone_number { separated.push("phone_number = ").push_bind(p.as_str().to_string()); }
            if let Some(v) = params.phone_verified { separated.push("phone_verified = ").push_bind(v); }
            if let Some(s) = params.account_state { separated.push("account_state = ").push_bind(s.as_str().to_string()); }
            if let Some(b) = params.birth_date { separated.push("birth_date = ").push_bind(b.value()); }
            if let Some(l) = params.locale { separated.push("locale = ").push_bind(l.as_str().to_string()); }

            separated.push("updated_at = NOW()");
            qb.push(" WHERE id = ").push_bind(uid);

            qb.build().execute(conn).await.map_domain::<Account>()
        }))
            .await?;

        Ok(())
    }

    async fn save(&self, user: &Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.execute_upsert(user, tx).await
    }

    async fn update_account_status_by_id(&self, id: &AccountId, state: AccountState, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();
        let state_str = state.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| Box::pin(async move {
            query("UPDATE users SET account_state = $1, updated_at = NOW() WHERE id = $2")
                .bind(state_str)
                .bind(uid)
                .execute(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;
        Ok(())
    }

    async fn update_account_last_active(&self, id: &AccountId) -> Result<()> {
        let uid = id.as_uuid();
        <dyn Transaction>::execute_on(&self.pool, None, |conn| Box::pin(async move {
            query("UPDATE users SET last_active_at = NOW() WHERE id = $1")
                .bind(uid)
                .execute(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();
        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| Box::pin(async move {
            query("DELETE FROM users WHERE id = $1")
                .bind(uid)
                .execute(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;
        Ok(())
    }
}

impl PostgresAccountRepository {
    async fn execute_upsert(&self, user: &Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let uid = user.id.as_uuid();
        let region = user.region_code.as_str().to_string();
        let ext_id = user.external_id.as_str().to_string();
        let username = user.username.as_str().to_string();
        let email = user.email.as_str().to_string();
        let email_v = user.email_verified;
        let phone = user.phone_number.as_ref().map(|p| p.as_str().to_string());
        let phone_v = user.phone_verified;
        let state = user.account_state.as_str().to_string();
        let birth = user.birth_date.as_ref().map(|d| d.value());
        let locale = user.locale.as_str().to_string();
        let updated = user.updated_at;

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            let sql = r#"
                INSERT INTO users (
                    id, region_code, external_id, username, email, email_verified,
                    phone_number, phone_verified, account_state, birth_date,
                    locale, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id) DO UPDATE SET
                    email = EXCLUDED.email,
                    email_verified = EXCLUDED.email_verified,
                    phone_number = EXCLUDED.phone_number,
                    phone_verified = EXCLUDED.phone_verified,
                    account_state = EXCLUDED.account_state,
                    locale = EXCLUDED.locale,
                    updated_at = EXCLUDED.updated_at
            "#;

            query(sql)
                .bind(uid).bind(region).bind(ext_id).bind(username).bind(email)
                .bind(email_v).bind(phone).bind(phone_v).bind(state).bind(birth)
                .bind(locale).bind(updated)
                .execute(conn)
                .await
                .map_domain::<Account>()
        }))
            .await?;

        Ok(())
    }
}