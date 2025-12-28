// crates/identity/src/value_objects/phone_number.rs

use crate::{DomainError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    pub fn new(phone: &str) -> Result<Self> {
        let phone = phone.trim();

        // Validation basique E.164 : commence par +, suivi de 10 à 15 chiffres
        if !phone.starts_with('+') {
            return Err(DomainError::Validation {
                field: "phone_number",
                reason: "must start with country code (+33, +1, etc.)".into(),
            });
        }

        let digits: String = phone[1..].chars().filter(|c| c.is_digit(10)).collect();
        if digits.len() < 10 || digits.len() > 15 {
            return Err(DomainError::Validation {
                field: "phone_number",
                reason: "must have 10 to 15 digits after country code".into(),
            });
        }

        Ok(Self(phone.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}