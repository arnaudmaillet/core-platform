use async_trait::async_trait;
use tracing::instrument;

use postgres_storage::{StorageError, TransactionManager};

use crate::application::port::account_repository::AccountRepository;
use crate::domain::aggregate::account::Account;
use crate::domain::value_object::{
    account_id::AccountId, account_status::AccountStatus, email_address::EmailAddress,
    identity_id::IdentityId,
};
use crate::error::AccountError;

use super::model::AccountRow;

/// PostgreSQL adapter for the [`AccountRepository`] port.
///
/// All single-entity writes and targeted reads are routed through
/// [`TransactionManager::run_on_shard`] keyed on [`AccountId`] — ensuring
/// that in `ApplicationSharded` topology every operation lands on the correct
/// shard pool while remaining completely transparent in `SingleNode` / distributed-table
/// mode (CockroachDB, Aurora).
///
/// Queries that are not scoped to a single account (list, count, email existence)
/// use [`TransactionManager::pool`], which is the correct approach for single-node
/// and CockroachDB deployments.
///
/// # Cross-shard fan-out
///
/// `find_by_identity_id`, `exists_by_email`, `list_by_status`, and `count_by_status`
/// are currently limited to the single-node pool. In a true `ApplicationSharded`
/// deployment where `identity_id` and `email` are the routing dimension, these
/// queries would require an auxiliary index table or a secondary consistent hash.
/// That routing strategy is intentionally deferred to the infrastructure evolution phase.
#[derive(Clone, Debug)]
pub struct PgAccountRepository {
    tx_manager: TransactionManager,
}

impl PgAccountRepository {
    pub fn new(tx_manager: TransactionManager) -> Self {
        Self { tx_manager }
    }
}

#[async_trait]
impl AccountRepository for PgAccountRepository {
    #[instrument(name = "account.repo.save", skip(self, account), fields(
        account.id = %account.id(),
        account.version = account.version(),
    ))]
    async fn save(&self, account: &Account) -> Result<(), AccountError> {
        let id = account.id();

        // Pre-materialize all account data as owned values so the async move closures
        // are 'static and do not borrow `account` across the await boundary.
        let p_identity_id        = account.identity_id().as_str().to_owned();
        let p_status             = account.status().as_str().to_owned();
        let p_suspension_reason  = account.suspension_reason().map(str::to_owned);
        let p_deactivated_at     = account.deactivated_at();
        let p_email              = account.email().as_str().to_owned();
        let p_email_verified     = account.email_verified();
        let p_email_verified_at  = account.email_verified_at();
        let p_phone              = account.phone().map(|p| p.as_str().to_owned());
        let p_phone_verified     = account.phone_verified();
        let p_phone_verified_at  = account.phone_verified_at();
        let p_password_hash      = account.password_hash().map(|h| h.as_str().to_owned());
        let p_password_changed   = account.password_changed_at();
        let p_failed_logins      = account.failed_login_attempts() as i16;
        let p_locked_until       = account.locked_until();
        let p_last_login         = account.last_login_at();
        let p_kyc_status         = account.kyc_status().as_str().to_owned();
        let p_kyc_reviewed_at    = account.kyc_reviewed_at();
        let p_kyc_reviewer_id    = account.kyc_reviewer_id().map(|r| r.as_uuid());
        let p_date_of_birth      = account.date_of_birth();
        let p_country            = account.country_of_residence().map(|c| c.as_str().to_owned());
        let p_created_at         = account.created_at();
        let p_updated_at         = account.updated_at();
        let p_created_by         = account.created_by().map(|c| c.as_uuid());

        let mfa = account.mfa();
        let p_mfa_enforced        = mfa.enforced();
        let p_mfa_totp_secret     = mfa.totp_secret().map(|s| s.as_bytes().to_vec());
        let p_mfa_enrolled_at     = mfa.totp_enrolled_at();
        let p_mfa_backup_at       = mfa.backup_verified_at();
        let p_recovery_codes: Vec<String> = mfa.recovery_codes().iter().map(|c| c.as_str().to_owned()).collect();

        let gdpr = account.gdpr();
        let p_gdpr_processing_at  = gdpr.data_processing_consented_at();
        let p_gdpr_marketing_at   = gdpr.marketing_consented_at();
        let p_gdpr_consent_ip     = gdpr.consent_ip_address().map(str::to_owned);
        let p_gdpr_consent_ver    = gdpr.last_consent_version().map(str::to_owned);
        let p_gdpr_deletion_req   = gdpr.deletion_requested_at();
        let p_gdpr_deletion_sched = gdpr.deletion_scheduled_at();
        let p_gdpr_anonymized     = gdpr.anonymized_at();
        let p_gdpr_export_req     = gdpr.data_export_requested_at();
        let p_gdpr_export_done    = gdpr.data_export_completed_at();

        let credit = account.credit();
        let p_credit_balance      = credit.balance_micros();
        let p_credit_reserved     = credit.reserved_micros();
        let p_credit_currency     = credit.currency().map(|c| c.as_str().to_owned());
        let p_credit_ledger_ver   = credit.ledger_version();
        let p_credit_last_tx_id   = credit.last_transaction_id().map(|t| t.as_uuid());
        let p_credit_last_tx_at   = credit.last_transaction_at();

        let p_roles: Vec<String> = account.roles().iter().map(|r| r.as_str().to_owned()).collect();
        let p_perms: Vec<String> = account.permission_overrides().to_vec();

        if account.version() == 0 {
            // New aggregate — INSERT.
            self.tx_manager
                .run_on_shard(&id, |tx| {
                    Box::pin(async move {
                        sqlx::query(
                            r#"
                            INSERT INTO accounts (
                                id, identity_id, status, suspension_reason, deactivated_at,
                                email, email_verified, email_verified_at,
                                phone, phone_verified, phone_verified_at,
                                password_hash, password_changed_at,
                                failed_login_attempts, locked_until, last_login_at,
                                mfa_enforced, mfa_totp_secret, mfa_totp_enrolled_at,
                                mfa_recovery_codes, mfa_backup_verified_at,
                                kyc_status, kyc_reviewed_at, kyc_reviewer_id,
                                date_of_birth, country_of_residence,
                                gdpr_data_processing_consented_at, gdpr_marketing_consented_at,
                                gdpr_consent_ip, gdpr_last_consent_version,
                                gdpr_deletion_requested_at, gdpr_deletion_scheduled_at,
                                gdpr_anonymized_at,
                                gdpr_data_export_requested_at, gdpr_data_export_completed_at,
                                roles, permission_overrides,
                                credit_balance, credit_reserved, credit_currency,
                                credit_ledger_version, credit_last_tx_id, credit_last_tx_at,
                                version, created_at, updated_at, created_by
                            ) VALUES (
                                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,
                                $17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,
                                $31,$32,$33,$34,$35,$36,$37,$38,$39,$40,$41,$42,$43,
                                $44,$45,$46,$47
                            )
                            "#,
                        )
                        .bind(id.as_uuid())
                        .bind(p_identity_id)
                        .bind(p_status)
                        .bind(p_suspension_reason)
                        .bind(p_deactivated_at)
                        .bind(p_email)
                        .bind(p_email_verified)
                        .bind(p_email_verified_at)
                        .bind(p_phone)
                        .bind(p_phone_verified)
                        .bind(p_phone_verified_at)
                        .bind(p_password_hash)
                        .bind(p_password_changed)
                        .bind(p_failed_logins)
                        .bind(p_locked_until)
                        .bind(p_last_login)
                        .bind(p_mfa_enforced)
                        .bind(p_mfa_totp_secret)
                        .bind(p_mfa_enrolled_at)
                        .bind(&p_recovery_codes)
                        .bind(p_mfa_backup_at)
                        .bind(p_kyc_status)
                        .bind(p_kyc_reviewed_at)
                        .bind(p_kyc_reviewer_id)
                        .bind(p_date_of_birth)
                        .bind(p_country)
                        .bind(p_gdpr_processing_at)
                        .bind(p_gdpr_marketing_at)
                        .bind(p_gdpr_consent_ip)
                        .bind(p_gdpr_consent_ver)
                        .bind(p_gdpr_deletion_req)
                        .bind(p_gdpr_deletion_sched)
                        .bind(p_gdpr_anonymized)
                        .bind(p_gdpr_export_req)
                        .bind(p_gdpr_export_done)
                        .bind(&p_roles)
                        .bind(&p_perms)
                        .bind(p_credit_balance)
                        .bind(p_credit_reserved)
                        .bind(p_credit_currency)
                        .bind(p_credit_ledger_ver)
                        .bind(p_credit_last_tx_id)
                        .bind(p_credit_last_tx_at)
                        .bind(1i64) // version starts at 1 after first save
                        .bind(p_created_at)
                        .bind(p_updated_at)
                        .bind(p_created_by)
                        .execute(&mut **tx)
                        .await
                        .map(|_| ())
                        .map_err(|e| AccountError::Storage(StorageError::from(e)))
                    })
                })
                .await
        } else {
            // Existing aggregate — UPDATE with optimistic CAS on version.
            let expected_version = account.version();

            self.tx_manager
                .run_on_shard(&id, |tx| {
                    Box::pin(async move {
                        let affected = sqlx::query(
                            r#"
                            UPDATE accounts SET
                                status = $2,
                                suspension_reason = $3,
                                deactivated_at = $4,
                                email = $5,
                                email_verified = $6,
                                email_verified_at = $7,
                                phone = $8,
                                phone_verified = $9,
                                phone_verified_at = $10,
                                password_hash = $11,
                                password_changed_at = $12,
                                failed_login_attempts = $13,
                                locked_until = $14,
                                last_login_at = $15,
                                mfa_enforced = $16,
                                mfa_totp_secret = $17,
                                mfa_totp_enrolled_at = $18,
                                mfa_recovery_codes = $19,
                                mfa_backup_verified_at = $20,
                                kyc_status = $21,
                                kyc_reviewed_at = $22,
                                kyc_reviewer_id = $23,
                                date_of_birth = $24,
                                country_of_residence = $25,
                                gdpr_data_processing_consented_at = $26,
                                gdpr_marketing_consented_at = $27,
                                gdpr_consent_ip = $28,
                                gdpr_last_consent_version = $29,
                                gdpr_deletion_requested_at = $30,
                                gdpr_deletion_scheduled_at = $31,
                                gdpr_anonymized_at = $32,
                                gdpr_data_export_requested_at = $33,
                                gdpr_data_export_completed_at = $34,
                                roles = $35,
                                permission_overrides = $36,
                                credit_balance = $37,
                                credit_reserved = $38,
                                credit_currency = $39,
                                credit_ledger_version = $40,
                                credit_last_tx_id = $41,
                                credit_last_tx_at = $42,
                                version = version + 1,
                                updated_at = NOW()
                            WHERE id = $1 AND version = $43
                            "#,
                        )
                        .bind(id.as_uuid())
                        .bind(p_status)
                        .bind(p_suspension_reason)
                        .bind(p_deactivated_at)
                        .bind(p_email)
                        .bind(p_email_verified)
                        .bind(p_email_verified_at)
                        .bind(p_phone)
                        .bind(p_phone_verified)
                        .bind(p_phone_verified_at)
                        .bind(p_password_hash)
                        .bind(p_password_changed)
                        .bind(p_failed_logins)
                        .bind(p_locked_until)
                        .bind(p_last_login)
                        .bind(p_mfa_enforced)
                        .bind(p_mfa_totp_secret)
                        .bind(p_mfa_enrolled_at)
                        .bind(&p_recovery_codes)
                        .bind(p_mfa_backup_at)
                        .bind(p_kyc_status)
                        .bind(p_kyc_reviewed_at)
                        .bind(p_kyc_reviewer_id)
                        .bind(p_date_of_birth)
                        .bind(p_country)
                        .bind(p_gdpr_processing_at)
                        .bind(p_gdpr_marketing_at)
                        .bind(p_gdpr_consent_ip)
                        .bind(p_gdpr_consent_ver)
                        .bind(p_gdpr_deletion_req)
                        .bind(p_gdpr_deletion_sched)
                        .bind(p_gdpr_anonymized)
                        .bind(p_gdpr_export_req)
                        .bind(p_gdpr_export_done)
                        .bind(&p_roles)
                        .bind(&p_perms)
                        .bind(p_credit_balance)
                        .bind(p_credit_reserved)
                        .bind(p_credit_currency)
                        .bind(p_credit_ledger_ver)
                        .bind(p_credit_last_tx_id)
                        .bind(p_credit_last_tx_at)
                        .bind(expected_version)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| AccountError::Storage(StorageError::from(e)))?
                        .rows_affected();

                        if affected == 0 {
                            Err(AccountError::ConcurrentModification)
                        } else {
                            Ok(())
                        }
                    })
                })
                .await
        }
    }

    #[instrument(name = "account.repo.find_by_id", skip(self), fields(
        account.id = %id,
    ))]
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, AccountError> {
        let pool = self
            .tx_manager
            .pool_for(id)
            .map_err(AccountError::Storage)?;

        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT * FROM accounts WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        row.map(Account::try_from).transpose()
    }

    #[instrument(name = "account.repo.find_by_identity_id", skip(self), fields(
        identity_id = %identity_id,
    ))]
    async fn find_by_identity_id(
        &self,
        identity_id: &IdentityId,
    ) -> Result<Option<Account>, AccountError> {
        // NOTE: Routing by identity_id is only correct in SingleNode / CockroachDB
        // topology. In ApplicationSharded mode, identity_id does not hash to the same
        // shard as AccountId, so this query would need to fan out across all shards or
        // use a secondary index table. For Phase 1 (single-node + CockroachDB), this is safe.
        let pool = self.tx_manager.pool();

        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT * FROM accounts WHERE identity_id = $1",
        )
        .bind(identity_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        row.map(Account::try_from).transpose()
    }

    #[instrument(name = "account.repo.list_by_status", skip(self), fields(
        status = %status,
        limit  = limit,
        offset = offset,
    ))]
    async fn list_by_status(
        &self,
        status: &AccountStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Account>, AccountError> {
        let pool = self.tx_manager.pool();

        let rows = sqlx::query_as::<_, AccountRow>(
            "SELECT * FROM accounts WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(status.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        rows.into_iter().map(Account::try_from).collect()
    }

    #[instrument(name = "account.repo.exists_by_email", skip(self), fields(email = %email))]
    async fn exists_by_email(&self, email: &EmailAddress) -> Result<bool, AccountError> {
        let pool = self.tx_manager.pool();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE email = $1")
                .bind(email.as_str())
                .fetch_one(pool)
                .await
                .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        Ok(count > 0)
    }

    #[instrument(name = "account.repo.exists_by_identity_id", skip(self), fields(
        identity_id = %identity_id,
    ))]
    async fn exists_by_identity_id(
        &self,
        identity_id: &IdentityId,
    ) -> Result<bool, AccountError> {
        let pool = self.tx_manager.pool();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE identity_id = $1")
                .bind(identity_id.as_str())
                .fetch_one(pool)
                .await
                .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        Ok(count > 0)
    }

    #[instrument(name = "account.repo.count_by_status", skip(self), fields(status = %status))]
    async fn count_by_status(&self, status: &AccountStatus) -> Result<i64, AccountError> {
        let pool = self.tx_manager.pool();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE status = $1")
                .bind(status.as_str())
                .fetch_one(pool)
                .await
                .map_err(|e| AccountError::Storage(StorageError::from(e)))?;

        Ok(count)
    }
}
