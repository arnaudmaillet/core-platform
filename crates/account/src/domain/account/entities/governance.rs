// crates/account/src/domain/entities/account_governance.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    domain::{
        entities::EntityMetadata,
        value_objects::{AccountId, AuditReason, TrustContext},
    },
    errors::Result,
};

use crate::domain::{
    account::builders::AccountGovernanceBuilder,
    value_objects::{AccountRole, IpAddr, TrustScore},
};

/// Entité Metadata (Interne à l'Agrégat Account)
///
/// Gère les scores de confiance, les rôles et les informations de modération.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountGovernance {
    account_id: AccountId,
    role: AccountRole,
    is_beta_tester: bool,
    is_shadowbanned: bool,
    trust_score: TrustScore,
    last_moderation_at: Option<DateTime<Utc>>,
    moderation_notes: Option<String>,
    last_ip_addr: Option<IpAddr>,
}

impl AccountGovernance {
    pub fn builder(account_id: AccountId) -> AccountGovernanceBuilder {
        AccountGovernanceBuilder::new(account_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        role: AccountRole,
        is_beta_tester: bool,
        is_shadowbanned: bool,
        trust_score: TrustScore,
        last_moderation_at: Option<DateTime<Utc>>,
        moderation_notes: Option<String>,
        last_ip_addr: Option<IpAddr>,
    ) -> Self {
        Self {
            account_id,
            role,
            is_beta_tester,
            is_shadowbanned,
            trust_score,
            last_moderation_at,
            moderation_notes,
            last_ip_addr,
        }
    }

    // --- GETTERS ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn role(&self) -> AccountRole {
        self.role
    }
    pub fn is_beta_tester(&self) -> bool {
        self.is_beta_tester
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

    pub(crate) fn apply_trust_reward(
        &mut self,
        amount: i32,
        context: TrustContext,
        reason: &AuditReason,
    ) -> Result<bool> {
        let previous_score = self.trust_score;
        self.trust_score.reward(amount);

        if self.trust_score == previous_score {
            return Ok(false);
        }

        // Log structuré : "Email verified: [SYSTEM] Automatic (Reward +5)"
        self.record_moderation_log(&format!("{}: {} (Reward +{})", context, reason, amount));
        Ok(true)
    }

    pub(crate) fn apply_trust_penalty(
        &mut self,
        amount: i32,
        context: TrustContext,
        reason: &AuditReason,
    ) -> Result<bool> {
        let previous_score = self.trust_score;
        self.trust_score.penalize(amount);

        let changed = self.trust_score != previous_score;

        if changed {
            self.record_moderation_log(&format!("{}: {} (Penalty -{})", context, reason, amount));
        }

        Ok(changed)
    }

    pub(crate) fn apply_shadowban(&mut self, reason: &AuditReason) -> Result<bool> {
        if self.is_shadowbanned {
            return Ok(false);
        }
        self.apply_shadowban_internal(&reason);
        Ok(true)
    }

    pub(crate) fn apply_lift_shadowban(&mut self, reason: &AuditReason) -> Result<bool> {
        if !self.is_shadowbanned {
            return Ok(false);
        }
        self.is_shadowbanned = false;
        self.record_moderation_log(&format!("Shadowban lifted: {}", reason));
        Ok(true)
    }

    pub(crate) fn apply_role_change(
        &mut self,
        new_role: AccountRole,
        reason: &AuditReason,
    ) -> Result<bool> {
        if self.role == new_role {
            return Ok(false);
        }
        self.role = new_role;
        self.record_moderation_log(&format!("Role changed to {:?}: {}", new_role, reason));
        Ok(true)
    }

    pub(crate) fn apply_beta_status(&mut self, status: bool, reason: &AuditReason) -> Result<bool> {
        if self.is_beta_tester == status {
            return Ok(false);
        }
        self.is_beta_tester = status;
        let action = if status { "enabled" } else { "disabled" };
        self.record_moderation_log(&format!("Beta status {}: {}", action, reason));
        Ok(true)
    }

    pub(crate) fn apply_ip_record(&mut self, ip: IpAddr) {
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

impl EntityMetadata for AccountGovernance {
    fn entity_name() -> &'static str {
        "AccountGovernance"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_governance_pkey" => "account_id",
            _ => "internal_governance",
        }
    }
}
