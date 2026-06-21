use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use uuid::Uuid;

use crate::application::port::AccountRepository;
use crate::domain::value_object::AccountId;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct CreditBalanceView {
    pub account_id: String,
    /// Current balance in micro-units (6 decimal places; e.g. 1 USD = 1_000_000).
    pub balance: i64,
    /// Amount reserved for pending settlements.
    pub reserved: i64,
    /// `balance - reserved` — spendable without waiting for settlement.
    pub available: i64,
    /// ISO 4217 currency code; `None` until first credit operation.
    pub currency: Option<String>,
    /// Monotonically incremented on every ledger write; used for optimistic locking.
    pub ledger_version: i64,
}

#[derive(Debug, Clone)]
pub struct GetCreditBalanceQuery {
    pub account_id: String,
}

impl Query for GetCreditBalanceQuery {
    type Response = CreditBalanceView;
}

pub struct GetCreditBalanceHandler {
    repo: Arc<dyn AccountRepository>,
}

impl GetCreditBalanceHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<GetCreditBalanceQuery> for GetCreditBalanceHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<GetCreditBalanceQuery>,
    ) -> Result<CreditBalanceView, Self::Error> {
        let id_str = &envelope.payload.account_id;
        let uuid = id_str.parse::<Uuid>().map_err(|_| AccountError::DomainViolation {
            field: "account_id".into(),
            message: "invalid UUID format".into(),
        })?;
        let id = AccountId::from_uuid(uuid);
        let account = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound { id: id_str.clone() })?;

        let credit = account.credit();
        Ok(CreditBalanceView {
            account_id: id_str.clone(),
            balance: credit.balance_micros(),
            reserved: credit.reserved_micros(),
            available: credit.available_micros(),
            currency: credit.currency().map(|c| c.as_str().to_owned()),
            ledger_version: credit.ledger_version(),
        })
    }
}
pub type GetCreditBalanceResponse = CreditBalanceView;
