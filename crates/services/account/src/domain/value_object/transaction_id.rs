use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// External ledger transaction reference.
///
/// Used as an idempotency key for credit/debit operations: if the same
/// `TransactionId` is applied twice, the repository adapter can detect the
/// duplicate and skip re-application without double-crediting the account.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransactionId(Uuid);

impl TransactionId {
    /// Generates a fresh UUIDv7 transaction identifier.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for TransactionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}
