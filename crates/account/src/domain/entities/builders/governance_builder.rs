// crates/account/src/domain/builders/account_governance_builder.rs

use crate::domain::entities::AccountGovernance;
use crate::domain::types::{AccountRole, IpAddr, TrustScore};
use crate::types::BetaTier;
use chrono::Utc;
use shared_kernel::types::AccountId;
use shared_kernel::core::Result;

pub struct AccountGovernanceBuilder {
    account_id: AccountId,
    role: AccountRole,
    trust_score: TrustScore,
    is_shadowbanned: bool,
    beta_tier: BetaTier,
    last_ip_addr: Option<IpAddr>,
}

impl AccountGovernanceBuilder {
    pub(crate) fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            role: AccountRole::USER,
            trust_score: TrustScore::new_max(),
            is_shadowbanned: false,
            beta_tier: BetaTier::NONE,
            last_ip_addr: None,
        }
    }

    // --- SETTERS ---

    pub fn with_role(mut self, role: AccountRole) -> Self {
        self.role = role;
        self
    }

    pub fn with_ip_addr(mut self, ip: IpAddr) -> Self {
        self.last_ip_addr = Some(ip);
        self
    }

    pub fn with_shadowban(mut self, is_shadowbanned: bool) -> Self {
        self.is_shadowbanned = is_shadowbanned;
        self
    }

    pub fn with_trust_score(mut self, score: TrustScore) -> Self {
        self.trust_score = score;
        self
    }

    pub fn build(self) -> Result<AccountGovernance> {
        let now = Utc::now();

        Ok(AccountGovernance::restore(
            self.account_id,
            self.role,
            self.beta_tier,
            self.is_shadowbanned,
            self.trust_score,
            None,
            Some(format!(
                "[{}] Account governance initialized.",
                now.format("%Y-%m-%d %H:%M:%S")
            )),
            self.last_ip_addr,
            now,
        ))
    }
}
