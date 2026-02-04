// crates/account/src/infrastructure/postgres/repositories/account_repository

use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Username};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::{Pool, Postgres, QueryBuilder, query, query_as, query_scalar};

use crate::domain::entities::Account;
use crate::domain::params::PatchUserParams;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};
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
        let id = <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM accounts WHERE email = $1")
                    .bind(email_str)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<Account>()
            })
        }).await?;
        Ok(id.map(AccountId::from_uuid))
    }

    async fn find_account_id_by_username(&self, username: &Username) -> Result<Option<AccountId>> {
        let username_str = username.as_str().to_string();

        let id = <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM accounts WHERE username = $1")
                    .bind(username_str)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        Ok(id.map(AccountId::from_uuid))
    }

    async fn find_account_id_by_external_id(
        &self,
        external_id: &ExternalId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        let ext_id = external_id.as_str().to_string();

        // 2. On passe 'tx' au lieu de 'None' dans execute_on
        let id = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, uuid::Uuid>("SELECT id FROM accounts WHERE external_id = $1")
                    .bind(ext_id)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        Ok(id.map(AccountId::from_uuid))
    }

    async fn find_account_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let uid = id.as_uuid();
        let has_tx = tx.is_some();

        let row = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = if has_tx {
                    "SELECT * FROM accounts WHERE id = $1 FOR UPDATE"
                } else {
                    "SELECT * FROM accounts WHERE id = $1"
                };

                query_as::<_, PostgresAccountRow>(sql)
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        row.map(Account::try_from).transpose()
    }

    // --- VÉRIFICATIONS ---

    async fn exists_account_by_email(&self, email: &Email) -> Result<bool> {
        let email_str = email.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, bool>(
                    "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = $1)",
                )
                .bind(email_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
            })
        })
        .await
    }

    async fn exists_account_by_username(&self, username: &Username) -> Result<bool> {
        let username_str = username.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, bool>(
                    "SELECT EXISTS(SELECT 1 FROM accounts WHERE username = $1)",
                )
                .bind(username_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
            })
        })
        .await
    }

    async fn exists_account_by_phone_number(&self, phone: &PhoneNumber) -> Result<bool> {
        let phone_str = phone.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query_scalar::<Postgres, bool>(
                    "SELECT EXISTS(SELECT 1 FROM accounts WHERE phone_number = $1)",
                )
                .bind(phone_str)
                .fetch_one(conn)
                .await
                .map_domain::<Account>()
            })
        })
        .await
    }

    // --- ÉCRITURES ---

    async fn create_account(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()> {
        let row = PostgresAccountRow::from(account);
        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query(
                    r#"
                    INSERT INTO accounts (
                        id, region_code, external_id, username, email,
                        phone_number, state, birth_date, locale,
                        created_at, updated_at, last_active_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                )
                    .bind(row.id)
                    .bind(&row.region_code)
                    .bind(&row.external_id)
                    .bind(&row.username)
                    .bind(&row.email)
                    .bind(&row.phone_number)
                    .bind(&row.state) // Colonne 'state'
                    .bind(row.birth_date)
                    .bind(&row.locale)
                    .bind(row.created_at)
                    .bind(row.updated_at)
                    .bind(row.last_active_at)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        }).await?;
        Ok(())
    }

    async fn patch_account_by_id(
        &self,
        id: &AccountId,
        params: PatchUserParams,
        tx: &mut dyn Transaction,
    ) -> Result<()> {
        if params.is_empty() {
            return Ok(());
        }
        let uid = id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                let mut qb = QueryBuilder::<Postgres>::new("UPDATE accounts SET ");

                {
                    let mut separated = qb.separated(", ");

                    if let Some(u) = params.username {
                        separated.push("username = ")
                            .push_bind_unseparated(u.as_str().to_string());
                    }
                    if let Some(e) = params.email {
                        separated.push("email = ")
                            .push_bind_unseparated(e.as_str().to_string());
                    }
                    if let Some(v) = params.email_verified {
                        separated.push("email_verified = ")
                            .push_bind_unseparated(v);
                    }
                    if let Some(p) = params.phone_number {
                        separated.push("phone_number = ")
                            .push_bind_unseparated(p.as_str().to_string());
                    }
                    if let Some(v) = params.phone_verified {
                        separated.push("phone_verified = ")
                            .push_bind_unseparated(v);
                    }
                    if let Some(s) = params.state {
                        separated.push("state = ")
                            .push_bind_unseparated(s.as_str().to_string());
                    }
                    if let Some(b) = params.birth_date {
                        separated.push("birth_date = ")
                            .push_bind_unseparated(b.value());
                    }
                    if let Some(l) = params.locale {
                        separated.push("locale = ")
                            .push_bind_unseparated(l.as_str().to_string());
                    }

                    separated.push("updated_at = NOW()");
                }

                qb.push(" WHERE id = ").push_bind(uid);

                qb.build().execute(conn).await.map_domain::<Account>()
            })
        })
            .await?;

        Ok(())
    }

    async fn save(&self, user: &Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.execute_upsert(user, tx).await
    }

    async fn update_account_status_by_id(
        &self,
        id: &AccountId,
        state: AccountState,
        tx: &mut dyn Transaction,
    ) -> Result<()> {
        let uid = id.as_uuid();
        let state_str = state.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("UPDATE accounts SET state = $1, updated_at = NOW() WHERE id = $2")
                    .bind(state_str)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;
        Ok(())
    }

    async fn update_account_last_active(&self, id: &AccountId) -> Result<()> {
        let uid = id.as_uuid();
        <dyn Transaction>::execute_on(&self.pool, None, |conn| {
            Box::pin(async move {
                query("UPDATE accounts SET last_active_at = NOW() WHERE id = $1")
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;
        Ok(())
    }

    async fn delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();
        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("DELETE FROM accounts WHERE id = $1")
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;
        Ok(())
    }
}

impl PostgresAccountRepository {
    async fn execute_upsert(&self, user: &Account, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let row = PostgresAccountRow::from(user);

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
            INSERT INTO accounts (
                id, region_code, external_id, username, email, email_verified,
                phone_number, phone_verified, state, birth_date,
                locale, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                email = EXCLUDED.email,
                email_verified = EXCLUDED.email_verified,
                phone_number = EXCLUDED.phone_number,
                phone_verified = EXCLUDED.phone_verified,
                state = EXCLUDED.state,
                locale = EXCLUDED.locale,
                updated_at = EXCLUDED.updated_at
        "#;

                // 2. On bind directement les champs de la row
                query(sql)
                    .bind(row.id)
                    .bind(&row.region_code)
                    .bind(&row.external_id)
                    .bind(&row.username)
                    .bind(&row.email)
                    .bind(row.email_verified)
                    .bind(&row.phone_number)
                    .bind(row.phone_verified)
                    .bind(&row.state) // Utilise le type PostgresAccountState de la row
                    .bind(row.birth_date)
                    .bind(&row.locale)
                    .bind(row.updated_at)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        Ok(())
    }
}
