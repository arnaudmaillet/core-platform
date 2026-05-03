// crates/account/src/infrastructure/postgres/rows/postgres_account_governance_row.rs

use chrono::{DateTime, Utc};
use std::net::IpAddr as StdIpAddr;

use crate::infrastructure::postgres::models::PostgresAccountRole;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountGovernanceRow {
    pub role: PostgresAccountRole,
    pub is_beta_tester: bool,
    pub is_shadowbanned: bool,
    pub trust_score: i32,
    pub last_moderation_at: Option<DateTime<Utc>>,
    pub moderation_notes: Option<String>,
    pub last_ip_addr: Option<StdIpAddr>,
    // pub account_id: Uuid,
    // #[sqlx(rename = "governance_updated_at")]
    // pub updated_at: DateTime<Utc>,
}

impl PostgresAccountGovernanceRow {
    pub fn from_domain(account: &crate::domain::account::entities::Account) -> Self {
        let gov = account.governance();

        Self {
            role: gov.role().into(),
            is_beta_tester: gov.is_beta_tester(),
            is_shadowbanned: gov.is_shadowbanned(),
            trust_score: gov.trust_score().value(),
            last_moderation_at: gov.last_moderation_at(),
            moderation_notes: gov.moderation_notes().map(|s| s.to_string()),
            last_ip_addr: gov.last_ip_addr().map(|ip| ip.to_std()),
            // account_id: account.identity().account_id().as_uuid(),
            // updated_at: gov.updated_at(),
        }
    }
}
