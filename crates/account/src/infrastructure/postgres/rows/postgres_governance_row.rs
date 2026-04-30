// crates/account/src/infrastructure/postgres/rows/postgres_account_governance_row.rs

use chrono::{DateTime, Utc};
use shared_kernel::{
    domain::{Identifier, entities::Entity, value_objects::AccountId},
    errors::Result,
};
use std::net::IpAddr as StdIpAddr;
use uuid::Uuid;

use crate::{
    domain::{
        account::entities::AccountGovernance,
        value_objects::{AccountRole, IpAddr, TrustScore},
    },
    infrastructure::postgres::models::PostgresAccountRole,
};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountGovernanceRow {
    pub account_id: Uuid,
    pub role: PostgresAccountRole,
    pub is_beta_tester: bool,
    pub is_shadowbanned: bool,
    pub trust_score: i32,
    pub last_moderation_at: Option<DateTime<Utc>>,
    pub moderation_notes: Option<String>,
    pub last_ip_addr: Option<StdIpAddr>,
    #[sqlx(rename = "governance_updated_at")]
    pub updated_at: DateTime<Utc>,
}

impl PostgresAccountGovernanceRow {
    pub fn to_domain(self) -> Result<AccountGovernance> {
        let last_ip_addr = self.last_ip_addr.map(IpAddr::from_raw);

        Ok(AccountGovernance::restore(
            AccountId::from_uuid(self.account_id),
            AccountRole::from(self.role),
            self.is_beta_tester,
            self.is_shadowbanned,
            TrustScore::try_new(self.trust_score)?,
            self.last_moderation_at,
            self.moderation_notes,
            last_ip_addr,
            self.updated_at
        ))
    }

    pub fn from_domain(account: &crate::domain::account::entities::Account) -> Self {
        let gov = account.governance();

        Self {
            account_id: account.identity().account_id().as_uuid(),
            role: gov.role().into(),
            is_beta_tester: gov.is_beta_tester(),
            is_shadowbanned: gov.is_shadowbanned(),
            trust_score: gov.trust_score().value(),
            last_moderation_at: gov.last_moderation_at(),
            moderation_notes: gov.moderation_notes().map(|s| s.to_string()),
            last_ip_addr: gov.last_ip_addr().map(|ip| ip.to_std()),
            updated_at: gov.updated_at()
        }
    }
}
