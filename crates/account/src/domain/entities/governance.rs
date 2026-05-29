// crates/account/src/domain/entities/account_governance.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Entity, Result},
    security::TrustContext,
    types::{AccountId, AuditReason},
};

use crate::{
    entities::AccountGovernanceBuilder,
    types::{AccountRole, BetaTier, IpAddr, TrustAmount, TrustScore},
};

/// Entité Metadata (Interne à l'Agrégat Account)
///
/// Gère les scores de confiance, les rôles et les informations de modération.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountGovernance {
    account_id: AccountId,
    role: AccountRole,
    beta_tier: BetaTier,
    is_shadowbanned: bool,
    trust_score: TrustScore,
    last_moderation_at: Option<DateTime<Utc>>,
    moderation_notes: Option<String>,
    last_ip_addr: Option<IpAddr>,
    updated_at: DateTime<Utc>,
}

impl AccountGovernance {
    pub fn builder(account_id: AccountId) -> AccountGovernanceBuilder {
        AccountGovernanceBuilder::new(account_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        account_id: AccountId,
        role: AccountRole,
        beta_tier: BetaTier,
        is_shadowbanned: bool,
        trust_score: TrustScore,
        last_moderation_at: Option<DateTime<Utc>>,
        moderation_notes: Option<String>,
        last_ip_addr: Option<IpAddr>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            account_id,
            role,
            beta_tier,
            is_shadowbanned,
            trust_score,
            last_moderation_at,
            moderation_notes,
            last_ip_addr,
            updated_at,
        }
    }

    // --- GETTERS ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn role(&self) -> AccountRole {
        self.role
    }
    pub fn beta_tier(&self) -> BetaTier {
        self.beta_tier
    }
    pub fn is_shadowbanned(&self) -> bool {
        self.is_shadowbanned
    }
    pub fn trust_score(&self) -> TrustScore {
        self.trust_score
    }
    pub fn last_moderation_at(&self) -> Option<DateTime<Utc>> {
        self.last_moderation_at
    }
    pub fn moderation_notes(&self) -> Option<&str> {
        self.moderation_notes.as_deref()
    }
    pub fn last_ip_addr(&self) -> Option<&IpAddr> {
        self.last_ip_addr.as_ref()
    }

    // ==========================================
    // MUTATIONS INTERNES (pub(crate))
    // ==========================================

    pub fn apply_trust_reward(
        &mut self,
        amount: TrustAmount,
        context: TrustContext,
        reason: &AuditReason,
    ) -> Result<bool> {
        let previous_score = self.trust_score;
        self.trust_score.reward(amount);

        if self.trust_score == previous_score {
            return Ok(false);
        }

        self.record_moderation_log(&format!("{}: {} (+{})", context, reason, amount));
        Ok(true)
    }

    pub fn apply_trust_penalty(
        &mut self,
        amount: TrustAmount,
        context: TrustContext,
        reason: &AuditReason,
    ) -> Result<bool> {
        let previous_score = self.trust_score;
        self.trust_score.penalize(amount);

        let changed = self.trust_score != previous_score;

        if changed {
            self.record_moderation_log(&format!("{}: {} (-{})", context, reason, amount));
        }

        Ok(changed)
    }

    pub fn apply_shadowban(&mut self, reason: &AuditReason) -> Result<bool> {
        if self.is_shadowbanned {
            return Ok(false);
        }
        self.apply_shadowban_internal(&reason);
        Ok(true)
    }

    pub fn apply_lift_shadowban(&mut self, reason: &AuditReason) -> Result<bool> {
        if !self.is_shadowbanned {
            return Ok(false);
        }
        self.is_shadowbanned = false;
        self.record_moderation_log(&format!("Shadowban lifted: {}", reason));
        Ok(true)
    }

    pub fn apply_role_change(
        &mut self,
        new_role: AccountRole,
        reason: &AuditReason,
    ) -> Result<bool> {
        if self.role == new_role {
            return Ok(false);
        }
        self.role = new_role;
        self.record_moderation_log(&format!(
            "Role changed to {:?}: {}",
            new_role.as_lowercase(),
            reason
        ));
        Ok(true)
    }

    pub fn apply_beta_tier_change(&mut self, new_tier: BetaTier) -> Result<bool> {
        if self.beta_tier == new_tier {
            return Ok(false);
        }
        let old_tier = self.beta_tier;
        self.beta_tier = new_tier;
        self.record_moderation_log(&format!(
            "Beta tier changed from {} to {}",
            old_tier.as_str(),
            new_tier.as_str()
        ));
        Ok(true)
    }

    pub fn apply_ip_record(&mut self, ip: IpAddr) {
        self.last_ip_addr = Some(ip);
    }

    // ==========================================
    // HELPERS PRIVÉS
    // ==========================================

    fn record_moderation_log(&mut self, log_entry: &str) {
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S");
        let new_note = format!("[{}] {}", timestamp, log_entry);

        if let Some(ref mut notes) = self.moderation_notes {
            notes.push_str(&format!("\n{}", new_note));
        } else {
            self.moderation_notes = Some(new_note);
        }
        self.last_moderation_at = Some(now);
    }

    fn apply_shadowban_internal(&mut self, reason: &AuditReason) {
        self.is_shadowbanned = true;
        self.record_moderation_log(&format!("Shadowbanned: {}", reason));
    }
}

impl Entity for AccountGovernance {
    type Id = AccountId;
    fn entity_name() -> &'static str {
        "AccountGovernance"
    }
    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_governance_pkey" => "account_id",
            _ => "internal_governance",
        }
    }
    fn id(&self) -> &Self::Id {
        &self.account_id
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}
