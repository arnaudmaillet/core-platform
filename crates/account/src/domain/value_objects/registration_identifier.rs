// domain/value_objects/registration_identifier.rs

use serde::{Deserialize, Serialize};
use shared_kernel::errors::{DomainError, Result};
use crate::domain::value_objects::{Email, PhoneNumber};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrationMethod {
    Email(Email),
    Phone(PhoneNumber),
    Both { email: Email, phone: PhoneNumber },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationIdentifier {
    method: RegistrationMethod,
}

impl RegistrationIdentifier {
    /// Constructeur via Email uniquement
    pub fn from_email(email: Email) -> Self {
        Self { method: RegistrationMethod::Email(email) }
    }

    /// Constructeur via Téléphone uniquement
    pub fn from_phone(phone: PhoneNumber) -> Self {
        Self { method: RegistrationMethod::Phone(phone) }
    }

    /// Constructeur avec les deux
    pub fn from_both(email: Email, phone: PhoneNumber) -> Self {
        Self { method: RegistrationMethod::Both { email, phone } }
    }

    pub fn try_from_phone(raw_phone: impl Into<String>) -> Result<Self> {
        let phone = PhoneNumber::try_new(raw_phone)?;
        Ok(Self::from_phone(phone))
    }

    pub fn try_from_email(raw_email: impl Into<String>) -> Result<Self> {
        // En supposant que Email a aussi un try_new()
        let email = Email::try_new(raw_email)?; 
        Ok(Self::from_email(email))
    }

    /// Constructeur "Smart" qui essaie de construire à partir d'options.
    /// C'est celui-ci que tu utiliseras dans tes contrôleurs/handlers.
    pub fn try_from_options(email: Option<Email>, phone: Option<PhoneNumber>) -> Result<Self> {
        match (email, phone) {
            (Some(e), Some(p)) => Ok(Self::from_both(e, p)),
            (Some(e), None) => Ok(Self::from_email(e)),
            (None, Some(p)) => Ok(Self::from_phone(p)),
            (None, None) => Err(DomainError::Validation {
                field: "registration_identifier",
                reason: "At least one registration method (email or phone) must be provided".into(),
            }),
        }
    }

    // --- Helpers d'accès ---

    pub fn email(&self) -> Option<&Email> {
        match &self.method {
            RegistrationMethod::Email(e) | RegistrationMethod::Both { email: e, .. } => Some(e),
            RegistrationMethod::Phone(_) => None,
        }
    }

    pub fn phone(&self) -> Option<&PhoneNumber> {
        match &self.method {
            RegistrationMethod::Phone(p) | RegistrationMethod::Both { phone: p, .. } => Some(p),
            RegistrationMethod::Email(_) => None,
        }
    }
}