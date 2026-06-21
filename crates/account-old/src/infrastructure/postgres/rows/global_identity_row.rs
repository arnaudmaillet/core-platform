// crates/account/src/infrastructure/postgres/rows/global_identity_row.rs

use crate::repositories::GlobalIdentityRegistration;
use crate::types::{AccountState, RegistrationIdentifier};
use chrono::{DateTime, Utc};
use infra_sqlx::sqlx;
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::{AccountId, Region, SubId};
use shared_kernel::types::{Email, Phone};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostgresGlobalIdentityRow {
    pub account_id: Uuid,
    pub region: String,
    pub sub_id: Option<String>,
    pub email_hash: Option<Vec<u8>>,
    pub phone_hash: Option<Vec<u8>>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PostgresGlobalIdentityRow {
    pub fn from_domain(domain: &GlobalIdentityRegistration) -> Self {
        Self {
            account_id: domain.account_id.uuid(),
            region: domain.region.as_str().to_string(),
            sub_id: domain.sub_id.as_ref().map(|s| s.as_str().to_string()),
            email_hash: domain.identifiers.email_hash(),
            phone_hash: domain.identifiers.phone_hash(),
            state: domain.state.as_str().to_string(),
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }

    pub fn to_domain(self) -> Result<GlobalIdentityRegistration> {
        let account_id = AccountId::from_uuid(self.account_id);
        let region = Region::try_from(self.region.as_str())?;
        let sub_id = self.sub_id.map(|s| SubId::try_new(&s)).transpose()?;
        let state = AccountState::from_str(&self.state)?;

        let identifiers = match (self.email_hash.is_some(), self.phone_hash.is_some()) {
            (true, true) => {
                let email = Email::try_new("placeholder@global.registry")?;
                let phone = Phone::try_new("+00000000000")?;
                RegistrationIdentifier::from_both(email, phone)
            }
            (true, false) => {
                let email = Email::try_new("placeholder@global.registry")?;
                RegistrationIdentifier::from_email(email)
            }
            (false, true) => {
                let phone = Phone::try_new("+00000000000")?;
                RegistrationIdentifier::from_phone(phone)
            }
            (false, false) => {
                if sub_id.is_some() {
                    let email = Email::try_new("oauth2@global.registry")?;
                    RegistrationIdentifier::from_email(email)
                } else {
                    return Err(Error::database(
                        "Global row invariant broken: No identifiers found",
                    ));
                }
            }
        };

        Ok(GlobalIdentityRegistration {
            account_id,
            region,
            sub_id,
            identifiers,
            state,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}
