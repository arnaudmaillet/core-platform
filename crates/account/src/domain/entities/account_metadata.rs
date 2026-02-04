// crates/account/src/domain/entities/account_metadata.rs

use crate::domain::builders::AccountMetadataBuilder;
use crate::domain::events::AccountEvent;
use crate::domain::value_objects::AccountRole;
use chrono::{DateTime, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AccountMetadata {
    account_id: AccountId,
    region_code: RegionCode,
    role: AccountRole,
    is_beta_tester: bool,
    is_shadowbanned: bool,
    trust_score: i32,
    last_moderation_at: Option<DateTime<Utc>>,
    moderation_notes: Option<String>,
    estimated_ip: Option<String>,
    updated_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl AccountMetadata {
    pub fn builder(account_id: AccountId, region_code: RegionCode) -> AccountMetadataBuilder {
        AccountMetadataBuilder::new(account_id, region_code)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        role: AccountRole,
        is_beta_tester: bool,
        is_shadowbanned: bool,
        trust_score: i32,
        last_moderation_at: Option<DateTime<Utc>>,
        moderation_notes: Option<String>,
        estimated_ip: Option<String>,
        updated_at: DateTime<Utc>,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            account_id,
            region_code,
            role,
            is_beta_tester,
            is_shadowbanned,
            trust_score,
            last_moderation_at,
            moderation_notes,
            estimated_ip,
            updated_at,
            metadata,
        }
    }

    // --- GETTERS PUBLICS ---

    pub fn account_id(&self) -> &AccountId { &self.account_id }
    pub fn region_code(&self) -> &RegionCode { &self.region_code }
    pub fn role(&self) -> AccountRole { self.role }
    pub fn is_beta_tester(&self) -> bool { self.is_beta_tester }
    pub fn is_shadowbanned(&self) -> bool { self.is_shadowbanned }
    pub fn trust_score(&self) -> i32 { self.trust_score }
    pub fn last_moderation_at(&self) -> Option<DateTime<Utc>> { self.last_moderation_at }
    pub fn moderation_notes(&self) -> Option<&str> { self.moderation_notes.as_deref() }
    pub fn estimated_ip(&self) -> Option<&str> { self.estimated_ip.as_deref() }
    pub fn updated_at(&self) -> DateTime<Utc> { self.updated_at }

    // ==========================================
    // LOGIQUE DE MODÉRATION & SCORE
    // ==========================================

    /// Ajuste le score de confiance. Un score trop bas pourrait déclencher
    /// des restrictions automatiques via le Use Case.
    pub fn increase_trust_score(&mut self, action_id: Uuid, amount: u32, reason: String) {
        let delta = amount as i32;
        self.trust_score += delta;
        self.apply_moderation_change(format!("Score increased by {}: {}", amount, reason));

        self.add_event(Box::new(AccountEvent::TrustScoreAdjusted {
            id: action_id,
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            delta,
            new_score: self.trust_score,
            reason,
            occurred_at: self.updated_at,
        }));
    }

    /// Sanctionne un comportement négatif
    pub fn decrease_trust_score(&mut self, action_id: Uuid, amount: u32, reason: String) {
        let delta = -(amount as i32);
        self.trust_score += delta;
        self.apply_moderation_change(format!("Score decreased by {}: {}", amount, reason));

        self.add_event(Box::new(AccountEvent::TrustScoreAdjusted {
            id: action_id,
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            delta,
            new_score: self.trust_score,
            reason: reason.clone(),
            occurred_at: self.updated_at,
        }));

        // Règle métier Hyperscale : Auto-shadowban si le score chute trop bas
        if self.trust_score < -20 && !self.is_shadowbanned {
            self.apply_shadowban(format!(
                "Automated system: Trust score critical ({})",
                self.trust_score
            ));
        }
    }

    pub fn shadowban(&mut self, reason: String) {
        if !self.is_shadowbanned {
            self.apply_shadowban(reason);
        }
    }

    pub fn lift_shadowban(&mut self, reason: String) {
        if self.is_shadowbanned {
            self.is_shadowbanned = false;
            self.apply_moderation_change(format!("Shadowban lifted: {}", reason));

            self.add_event(Box::new(AccountEvent::ShadowbanStatusChanged {
                account_id: self.account_id.clone(),
                region: self.region_code.clone(),
                is_shadowbanned: false,
                reason,
                occurred_at: self.updated_at,
            }));
        }
    }

    /// Change le rôle du compte (Admin only via Use Case)
    pub fn upgrade_role(&mut self, new_role: AccountRole, reason: String) -> Result<()> {
        // 1. Idempotence : si le rôle est déjà le bon, on ne fait rien
        if self.role == new_role {
            return Ok(());
        }

        let old_role = self.role;
        self.role = new_role;

        // 2. Traçabilité interne
        self.apply_moderation_change(format!("Role changed to {:?}: {}", new_role, reason));

        // 3. Capture de l'événement
        self.add_event(Box::new(AccountEvent::AccountRoleChanged {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            old_role,
            new_role,
            reason,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn is_high_trust(&self) -> bool {
        self.trust_score > 100 && !self.is_shadowbanned
    }

    pub fn is_staff(&self) -> bool {
        self.role.has_permission_of(AccountRole::Staff)
    }

    pub fn set_beta_status(&mut self, status: bool, reason: String) {
        if self.is_beta_tester == status {
            return;
        }

        self.is_beta_tester = status;
        let action = if status { "enabled" } else { "disabled" };

        self.apply_moderation_change(format!("Beta tester mode {}: {}", action, reason));

        self.add_event(Box::new(AccountEvent::BetaStatusChanged {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            is_beta_tester: status,
            occurred_at: self.updated_at,
        }));
    }

    // ==========================================
    // LOGIQUE DE SHARDING (GÉOGRAPHIE)
    // ==========================================

    /// Change la région du compte.
    /// ATTENTION: cela implique souvent une migration physique des données.
    pub fn change_region(&mut self, new_region: RegionCode) -> Result<()> {
        if self.region_code == new_region {
            return Ok(());
        }

        let old_region = self.region_code.clone();
        self.region_code = new_region.clone();
        self.updated_at = Utc::now();

        self.add_event(Box::new(AccountEvent::AccountRegionChanged {
            account_id: self.account_id.clone(),
            old_region,
            new_region,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    // --- LOGIQUE DE VERSIONING ---

    fn apply_change(&mut self) {
        self.increment_version(); // Méthode de AggregateRoot
        self.updated_at = Utc::now();
    }

    // ==========================================
    // HELPERS PRIVÉS
    // ==========================================

    fn add_moderation_note(&mut self, note: String) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");
        let new_note = format!("[{}] {}", timestamp, note);

        if let Some(ref mut existing) = self.moderation_notes {
            existing.push_str(&format!("\n{}", new_note));
        } else {
            self.moderation_notes = Some(new_note);
        }
    }

    fn apply_moderation_change(&mut self, log_entry: String) {
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S");
        let new_note = format!("[{}] {}", timestamp, log_entry);

        if let Some(ref mut notes) = self.moderation_notes {
            notes.push_str(&format!("\n{}", new_note));
        } else {
            self.moderation_notes = Some(new_note);
        }

        self.last_moderation_at = Some(now);
        self.updated_at = now;
    }

    fn apply_shadowban(&mut self, reason: String) {
        self.is_shadowbanned = true;
        self.apply_moderation_change(format!("Shadowbanned: {}", reason));

        self.add_event(Box::new(AccountEvent::ShadowbanStatusChanged {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            is_shadowbanned: true,
            reason,
            occurred_at: self.updated_at,
        }));
    }
}

impl EntityMetadata for AccountMetadata {
    fn entity_name() -> &'static str {
        "AccountMetadata"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "user_internal_metadata_pkey" => "account_id",
            _ => "internal_metadata",
        }
    }
}

impl AggregateRoot for AccountMetadata {
    fn id(&self) -> String {
        self.account_id.as_string()
    }

    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
