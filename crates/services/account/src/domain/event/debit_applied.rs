use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, CreditAmount, CurrencyCode, TransactionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebitApplied {
    pub account_id: AccountId,
    pub amount: CreditAmount,
    pub currency: CurrencyCode,
    pub transaction_id: TransactionId,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
