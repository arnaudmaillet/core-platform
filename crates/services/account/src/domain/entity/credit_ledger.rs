use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{CreditAmount, CurrencyCode, TransactionId};
use crate::error::AccountError;

/// In-account financial ledger state.
///
/// Tracks the available credit balance, reservations, and a lightweight
/// audit trail (last transaction ID + timestamp). This is an **embedded
/// entity** inside the [`Account`] aggregate — it has its own optimistic
/// lock counter (`ledger_version`) so that financial writes (credit/debit)
/// and non-financial writes (KYC update, role assignment) can be applied
/// concurrently without false conflicts.
///
/// Currency is immutable once set — a currency change would require a
/// separate ledger migration that is outside this bounded context.
///
/// [`Account`]: crate::domain::aggregate::account::Account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditLedger {
    /// Current total balance in micro-units of [`currency`].
    ///
    /// [`currency`]: Self::currency
    pub balance: CreditAmount,

    /// Portion of [`balance`] currently reserved pending settlement.
    /// `balance - reserved = available for immediate use`.
    ///
    /// [`balance`]: Self::balance
    pub reserved: CreditAmount,

    /// ISO 4217 currency code. `None` until the first credit operation sets it.
    pub currency: Option<CurrencyCode>,

    /// Monotonically incremented on every financial write. Used in
    /// `WHERE ledger_version = $n` optimistic-lock checks.
    pub ledger_version: i64,

    /// Idempotency key of the last applied transaction.
    pub last_transaction_id: Option<TransactionId>,

    /// Wall-clock time of the last applied transaction.
    pub last_transaction_at: Option<DateTime<Utc>>,
}

impl CreditLedger {
    /// Reconstructs a ledger from raw persistence values (no events emitted).
    ///
    /// `balance_micros` and `reserved_micros` must be non-negative — the same
    /// invariant enforced by [`CreditAmount::from_micro`].
    pub fn reconstitute(
        balance_micros: i64,
        reserved_micros: i64,
        currency: Option<CurrencyCode>,
        ledger_version: i64,
        last_transaction_id: Option<TransactionId>,
        last_transaction_at: Option<DateTime<Utc>>,
    ) -> Result<Self, crate::error::AccountError> {
        Ok(Self {
            balance: CreditAmount::from_micro(balance_micros)?,
            reserved: CreditAmount::from_micro(reserved_micros)?,
            currency,
            ledger_version,
            last_transaction_id,
            last_transaction_at,
        })
    }

    /// Raw micro-unit value of the total balance.
    pub fn balance_micros(&self) -> i64 { self.balance.as_micro() }

    /// Raw micro-unit value of the reserved portion.
    pub fn reserved_micros(&self) -> i64 { self.reserved.as_micro() }

    /// Raw micro-unit value of the available (balance − reserved) amount.
    ///
    /// Returns zero if reserved exceeds balance (should not occur in a healthy ledger).
    pub fn available_micros(&self) -> i64 {
        (self.balance.as_micro() - self.reserved.as_micro()).max(0)
    }

    /// Returns the ISO 4217 currency code, or `None` if the ledger has no currency yet.
    pub fn currency(&self) -> Option<&CurrencyCode> { self.currency.as_ref() }

    /// Returns the current optimistic-lock version of the ledger.
    pub fn ledger_version(&self) -> i64 { self.ledger_version }

    /// Returns the idempotency key of the last applied transaction, if any.
    pub fn last_transaction_id(&self) -> Option<&TransactionId> { self.last_transaction_id.as_ref() }

    /// Returns the wall-clock time of the last applied transaction, if any.
    pub fn last_transaction_at(&self) -> Option<DateTime<Utc>> { self.last_transaction_at }

    /// Creates a fresh ledger denominated in `currency`.
    pub fn new(currency: CurrencyCode) -> Self {
        Self {
            balance: CreditAmount::zero(),
            reserved: CreditAmount::zero(),
            currency: Some(currency),
            ledger_version: 0,
            last_transaction_id: None,
            last_transaction_at: None,
        }
    }

    /// Returns the amount immediately available (balance minus reservations).
    pub fn available(&self) -> Result<CreditAmount, AccountError> {
        self.balance.checked_sub(&self.reserved)
    }

    /// Credits `amount` to the balance.
    pub fn credit(
        &mut self,
        amount: CreditAmount,
        tx_id: TransactionId,
    ) -> Result<(), AccountError> {
        self.balance = self.balance.checked_add(&amount)?;
        self.bump(tx_id);
        Ok(())
    }

    /// Debits `amount` from the available balance.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::InsufficientBalance`] if the available amount
    /// (balance − reserved) is less than `amount`.
    pub fn debit(
        &mut self,
        amount: CreditAmount,
        tx_id: TransactionId,
    ) -> Result<(), AccountError> {
        let available = self.available()?;
        available.checked_sub(&amount)?; // validate before mutating
        self.balance = self.balance.checked_sub(&amount)?;
        self.bump(tx_id);
        Ok(())
    }

    /// Moves `amount` from available to reserved, pending settlement.
    pub fn reserve(&mut self, amount: CreditAmount) -> Result<(), AccountError> {
        let available = self.available()?;
        available.checked_sub(&amount)?;
        self.reserved = self.reserved.checked_add(&amount)?;
        Ok(())
    }

    /// Releases a previously reserved amount back into the available pool.
    pub fn release_reservation(&mut self, amount: CreditAmount) -> Result<(), AccountError> {
        self.reserved = self.reserved.checked_sub(&amount)?;
        Ok(())
    }

    fn bump(&mut self, tx_id: TransactionId) {
        self.ledger_version += 1;
        self.last_transaction_id = Some(tx_id);
        self.last_transaction_at = Some(Utc::now());
    }
}

impl Default for CreditLedger {
    fn default() -> Self {
        Self {
            balance: CreditAmount::zero(),
            reserved: CreditAmount::zero(),
            currency: None,
            ledger_version: 0,
            last_transaction_id: None,
            last_transaction_at: None,
        }
    }
}
