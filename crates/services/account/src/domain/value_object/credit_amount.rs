use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// Fixed-point, non-negative monetary amount with 6 decimal places.
///
/// The inner `i64` stores the value in **micro-units**: `1_000_000` represents
/// exactly `1.000000` of the account's currency. This avoids all floating-point
/// rounding and matches `NUMERIC(18, 6)` in PostgreSQL directly.
///
/// The invariant `value >= 0` is enforced at construction and on every
/// arithmetic operation. Violating it returns `AccountError::InvalidCreditAmount`
/// rather than silently wrapping or panicking.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct CreditAmount(i64);

impl CreditAmount {
    /// The scale factor: 1 unit = 1_000_000 micro-units.
    pub const SCALE: i64 = 1_000_000;

    /// Zero amount.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Constructs from micro-units (the stored representation).
    ///
    /// # Errors
    ///
    /// Returns `AccountError::InvalidCreditAmount` if `micro < 0`.
    pub fn from_micro(micro: i64) -> Result<Self, AccountError> {
        if micro < 0 {
            return Err(AccountError::InvalidCreditAmount(format!(
                "credit amount must be non-negative (got {})",
                micro
            )));
        }
        Ok(Self(micro))
    }

    /// Returns the raw micro-unit value.
    pub fn as_micro(&self) -> i64 {
        self.0
    }

    /// Returns `true` if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Adds `other` to `self`.
    ///
    /// # Errors
    ///
    /// Returns `AccountError::ArithmeticOverflow` if the result would
    /// exceed `i64::MAX`.
    pub fn checked_add(&self, other: &Self) -> Result<Self, AccountError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(AccountError::ArithmeticOverflow)
    }

    /// Subtracts `other` from `self`.
    ///
    /// # Errors
    ///
    /// Returns `AccountError::InsufficientBalance` if the result would be
    /// negative (i.e. `other > self`).
    pub fn checked_sub(&self, other: &Self) -> Result<Self, AccountError> {
        match self.0.checked_sub(other.0) {
            Some(result) if result >= 0 => Ok(Self(result)),
            _ => Err(AccountError::InsufficientBalance),
        }
    }
}

impl fmt::Display for CreditAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / Self::SCALE;
        let frac  = (self.0 % Self::SCALE).abs();
        write!(f, "{}.{:06}", whole, frac)
    }
}

impl Default for CreditAmount {
    fn default() -> Self {
        Self::zero()
    }
}
