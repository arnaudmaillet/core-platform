// crates/account/src/domain/types/registration_identifier.rs

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared_kernel::{
    core::{Error, Result, ValueObject},
    types::{Email, Phone},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum RegistrationMethod {
    Email(Email),
    Phone(Phone),
    Both { email: Email, phone: Phone },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationIdentifier {
    method: RegistrationMethod,
}

impl RegistrationIdentifier {
    pub fn from_email(email: Email) -> Self {
        Self {
            method: RegistrationMethod::Email(email),
        }
    }

    pub fn from_phone(phone: Phone) -> Self {
        Self {
            method: RegistrationMethod::Phone(phone),
        }
    }

    pub fn from_both(email: Email, phone: Phone) -> Self {
        Self {
            method: RegistrationMethod::Both { email, phone },
        }
    }

    /// Constructeur robuste à partir d'options (Typique des handlers/endpoints gRPC)
    pub fn try_from_options(email: Option<Email>, phone: Option<Phone>) -> Result<Self> {
        match (email, phone) {
            (Some(e), Some(p)) => Ok(Self::from_both(e, p)),
            (Some(e), None) => Ok(Self::from_email(e)),
            (None, Some(p)) => Ok(Self::from_phone(p)),
            (None, None) => Err(Error::validation(
                "registration_identifier",
                "At least one valid identity identifier (email or phone) must be provided",
            )),
        }
    }

    // --- ACCESSEURS DE SÉCURITÉ ---

    pub fn email(&self) -> Option<&Email> {
        match &self.method {
            RegistrationMethod::Email(e) | RegistrationMethod::Both { email: e, .. } => Some(e),
            RegistrationMethod::Phone(_) => None,
        }
    }

    pub fn phone(&self) -> Option<&Phone> {
        match &self.method {
            RegistrationMethod::Phone(p) | RegistrationMethod::Both { phone: p, .. } => Some(p),
            RegistrationMethod::Email(_) => None,
        }
    }

    // --- 🔐 CAPACITÉS CRYPTOGRAPHIQUES DU DOMAINE ---

    /// Génère le hash SHA-256 binaire (32 octets) de l'e-mail de manière standardisée
    pub fn email_hash(&self) -> Option<Vec<u8>> {
        self.email().map(|e| {
            let mut hasher = Sha256::new();
            // Le Value Object Email nettoie déjà en principe sa chaîne (trim/lowercase)
            hasher.update(e.as_str().as_bytes());
            hasher.finalize().to_vec()
        })
    }

    /// Génère le hash SHA-256 binaire (32 octets) du téléphone de manière standardisée
    pub fn phone_hash(&self) -> Option<Vec<u8>> {
        self.phone().map(|p| {
            let mut hasher = Sha256::new();
            // Le type fort PhoneNumber garantit un format international standardisé (E.164)
            hasher.update(p.as_str().as_bytes());
            hasher.finalize().to_vec()
        })
    }
}

impl ValueObject for RegistrationIdentifier {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl fmt::Display for RegistrationIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.method {
            RegistrationMethod::Email(e) => write!(f, "Email({})", e),
            RegistrationMethod::Phone(p) => write!(f, "Phone({})", p),
            RegistrationMethod::Both { email, phone } => {
                write!(f, "Both(Email: {}, Phone: {})", email, phone)
            }
        }
    }
}
