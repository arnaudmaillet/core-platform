use async_trait::async_trait;
use postgres_storage::{StorageError, TransactionManager};
use tracing::instrument;

use crate::application::port::RefreshTokenRepository;
use crate::domain::aggregate::RefreshToken;
use crate::domain::value_object::{RefreshTokenHash, SessionId};
use crate::error::AuthError;

use super::model::RefreshTokenRow;

/// PostgreSQL adapter for [`RefreshTokenRepository`]. Writes route on `account_id`.
#[derive(Clone)]
pub struct PgRefreshTokenRepository {
    tx: TransactionManager,
}

impl PgRefreshTokenRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

fn storage(e: sqlx::Error) -> AuthError {
    AuthError::Storage(StorageError::from(e))
}

#[async_trait]
impl RefreshTokenRepository for PgRefreshTokenRepository {
    #[instrument(name = "auth.refresh.save", skip(self, token), fields(
        refresh.id = %token.id(), refresh.version = token.version()
    ))]
    async fn save(&self, token: &RefreshToken) -> Result<(), AuthError> {
        let account_id = token.account_id();
        let id = token.id().as_uuid();
        let session_id = token.session_id().as_uuid();
        let account_uuid = account_id.as_uuid();
        let hash = token.token_hash().as_str().to_owned();
        let status = token.status().as_str().to_owned();
        let replaced_by = token.replaced_by().map(|r| r.as_uuid());
        let token_issued_at = token.issued_at();
        let token_expires_at = token.expires_at();
        let token_used_at = token.used_at();
        let new_version = token.version();

        if new_version == 0 {
            self.tx
                .run_on_shard(&account_id, move |tx| {
                    Box::pin(async move {
                        sqlx::query(
                            r#"
                            INSERT INTO refresh_tokens (
                                id, session_id, account_id, token_hash, status,
                                issued_at, expires_at, used_at, replaced_by, version
                            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                            "#,
                        )
                        .bind(id)
                        .bind(session_id)
                        .bind(account_uuid)
                        .bind(hash)
                        .bind(status)
                        .bind(token_issued_at)
                        .bind(token_expires_at)
                        .bind(token_used_at)
                        .bind(replaced_by)
                        .bind(0i64)
                        .execute(&mut **tx)
                        .await
                        .map(|_| ())
                        .map_err(storage)
                    })
                })
                .await
        } else {
            self.tx
                .run_on_shard(&account_id, move |tx| {
                    Box::pin(async move {
                        let affected = sqlx::query(
                            r#"
                            UPDATE refresh_tokens SET
                                status = $2,
                                used_at = $3,
                                replaced_by = $4,
                                version = $5
                            WHERE id = $1 AND version = $6
                            "#,
                        )
                        .bind(id)
                        .bind(status)
                        .bind(token_used_at)
                        .bind(replaced_by)
                        .bind(new_version)
                        .bind(new_version - 1)
                        .execute(&mut **tx)
                        .await
                        .map_err(storage)?
                        .rows_affected();

                        if affected == 0 {
                            Err(AuthError::ConcurrentModification)
                        } else {
                            Ok(())
                        }
                    })
                })
                .await
        }
    }

    #[instrument(name = "auth.refresh.find_by_hash", skip(self, hash))]
    async fn find_by_hash(
        &self,
        hash: &RefreshTokenHash,
    ) -> Result<Option<RefreshToken>, AuthError> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT * FROM refresh_tokens WHERE token_hash = $1",
        )
        .bind(hash.as_str())
        .fetch_optional(self.tx.pool())
        .await
        .map_err(storage)?;
        row.map(RefreshToken::try_from).transpose()
    }

    #[instrument(name = "auth.refresh.revoke_all_for_session", skip(self), fields(session.id = %session_id))]
    async fn revoke_all_for_session(&self, session_id: &SessionId) -> Result<(), AuthError> {
        // Bulk infra operation keyed by session_id (not the shard key); single pool.
        sqlx::query(
            "UPDATE refresh_tokens SET status = 'revoked', used_at = NOW() \
             WHERE session_id = $1 AND status = 'active'",
        )
        .bind(session_id.as_uuid())
        .execute(self.tx.pool())
        .await
        .map_err(storage)?;
        Ok(())
    }
}
