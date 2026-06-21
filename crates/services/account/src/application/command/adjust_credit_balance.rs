use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use uuid::Uuid;
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::{CreditAmount, CurrencyCode, TransactionId};
use crate::error::AccountError;

/// Applies a signed credit delta to the account's ledger.
///
/// Positive `delta` → credit; negative `delta` → debit.
/// The `transaction_id` serves as an idempotency key — replaying the same
/// transaction ID must be a no-op at the domain layer.
#[derive(Debug, Clone)]
pub struct AdjustCreditBalanceCommand {
    pub account_id: String,
    /// Signed fixed-point delta in micro-units (6 decimal places, e.g. 1 USD = 1_000_000).
    pub delta: i64,
    /// ISO 4217 currency code (3 uppercase alpha chars).
    pub currency: String,
    /// UUID string of the external ledger transaction for idempotency.
    pub transaction_id: String,
}

impl Command for AdjustCreditBalanceCommand {}

impl Validate for AdjustCreditBalanceCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();

        let cur = self.currency.trim();
        if cur.len() != 3 || !cur.chars().all(|c| c.is_ascii_alphabetic()) {
            v.push(FieldViolation::new(
                "currency",
                "VAL-2050",
                "currency must be a 3-letter ISO 4217 code (e.g. USD)",
            ));
        }

        if self.transaction_id.parse::<Uuid>().is_err() {
            v.push(FieldViolation::new(
                "transaction_id",
                "VAL-2051",
                "transaction_id must be a valid UUID",
            ));
        }

        if self.delta == 0 {
            v.push(FieldViolation::new("delta", "VAL-2052", "delta must be non-zero"));
        }

        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct AdjustCreditBalanceHandler {
    repo: Arc<dyn AccountRepository>,
}

impl AdjustCreditBalanceHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<AdjustCreditBalanceCommand> for AdjustCreditBalanceHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<AdjustCreditBalanceCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let currency = CurrencyCode::new(cmd.currency.trim())?;

        let tx_uuid = cmd.transaction_id.trim().parse::<Uuid>().map_err(|_| {
            AccountError::DomainViolation {
                field: "transaction_id".into(),
                message: "invalid UUID format".into(),
            }
        })?;
        let transaction_id = TransactionId::from_uuid(tx_uuid);

        let amount = CreditAmount::from_micro(cmd.delta.abs())?;

        let mut account = load_account(&self.repo, &cmd.account_id).await?;

        if cmd.delta > 0 {
            account.apply_credit(amount, currency, transaction_id, envelope.correlation_id)?;
        } else {
            account.apply_debit(amount, currency, transaction_id, envelope.correlation_id)?;
        }

        self.repo.save(&account).await
    }
}
