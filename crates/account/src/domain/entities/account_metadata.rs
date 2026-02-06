// crates/account/src/domain/entities/account_metadata.rs

use crate::domain::builders::AccountMetadataBuilder;
use crate::domain::events::AccountEvent;
use crate::domain::value_objects::AccountRole;
use chrono::{DateTime, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};
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
    pub fn increase_trust_score(&mut self, region: &RegionCode, action_id: Uuid, amount: u32, reason: String) -> Result<bool> {
        self.ensure_region_match(region)?;
        let previous_score = self.trust_score;
        let delta = amount as i32;

        self.trust_score = (self.trust_score + delta).min(100);

        if self.trust_score == previous_score {
            return Ok(false);
        }

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

        Ok(true)
    }

    /// Sanctionne un comportement négatif
    pub fn decrease_trust_score(&mut self, region: &RegionCode, action_id: Uuid, amount: u32, reason: String) -> Result<bool> {
        self.ensure_region_match(region)?;

        let previous_score = self.trust_score;
        let delta = amount as i32;
        self.trust_score = (self.trust_score - delta).max(0);

        // Si le score n'a pas bougé (déjà à 0) ET que l'utilisateur est déjà shadowbanned
        // alors on a vraiment une opération idempotente (Ok(false))
        if self.trust_score == previous_score && self.is_shadowbanned {
            return Ok(false);
        }

        // Si le score a changé, on enregistre la note
        if self.trust_score != previous_score {
            self.apply_moderation_change(format!("Score decreased: {}", reason));
        }

        // Shadowban automatique si on tombe à zéro
        let mut shadowban_triggered = false;
        if self.trust_score == 0 && !self.is_shadowbanned {
            self.apply_shadowban("Automated system: Trust score dropped below critical threshold".into());
            shadowban_triggered = true;
        }

        // On n'ajoute l'événement de score que s'il y a eu un changement de score
        if self.trust_score != previous_score {
            self.add_event(Box::new(AccountEvent::TrustScoreAdjusted {
                id: action_id,
                account_id: self.account_id.clone(),
                region: self.region_code.clone(),
                delta: -(amount as i32),
                new_score: self.trust_score,
                reason,
                occurred_at: self.updated_at,
            }));
        }

        Ok(self.trust_score != previous_score || shadowban_triggered)
    }

    pub fn shadowban(&mut self, region: &RegionCode, reason: String) -> Result<bool>  {
        self.ensure_region_match(region)?;
        if !self.is_shadowbanned {
            self.apply_shadowban(reason);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn lift_shadowban(&mut self, region: &RegionCode, reason: String) -> Result<bool>  {
        self.ensure_region_match(region)?;
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
            return Ok(true);
        }
        Ok(false)
    }

    /// Change le rôle du compte (Admin only via Use Case)
    pub fn upgrade_role(&mut self, region: &RegionCode, new_role: AccountRole, reason: String) -> Result<bool> {
        self.ensure_region_match(region)?;

        // 1. Idempotence : si le rôle est déjà le bon, on ne fait rien
        if self.role == new_role {
            return Ok(false);
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

        Ok(true)
    }

    pub fn is_high_trust(&self) -> bool {
        self.trust_score > 100 && !self.is_shadowbanned
    }

    pub fn is_staff(&self) -> bool {
        self.role.has_permission_of(AccountRole::Staff)
    }

    pub fn set_beta_status(&mut self, region: &RegionCode, status: bool, reason: String) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.is_beta_tester == status {
            return Ok(false);
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

        Ok(true)
    }

    // ==========================================
    // LOGIQUE DE SHARDING (GÉOGRAPHIE)
    // ==========================================

    /// Change la région du compte.
    /// ATTENTION: cela implique souvent une migration physique des données.
    pub fn change_region(&mut self, new_region: RegionCode) -> Result<bool> {
        if self.region_code == new_region {
            return Ok(false);
        }

        let old_region = self.region_code.clone();
        self.region_code = new_region.clone();
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountRegionChanged {
            account_id: self.account_id.clone(),
            old_region,
            new_region,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    // ==========================================
    // HELPERS PRIVÉS
    // ==========================================

    fn apply_change(&mut self) {
        self.increment_version(); // Méthode de AggregateRoot
        self.updated_at = Utc::now();
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
        self.apply_change();
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

    fn ensure_region_match(&self, region: &RegionCode) -> Result<()> {
        if &self.region_code != region {
            return Err(DomainError::Forbidden {
                reason: "Cross-region operation detected".into(),
            });
        }
        Ok(())
    }
}

impl EntityMetadata for AccountMetadata {
    fn entity_name() -> &'static str {
        "AccountMetadata"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_metadata_pkey" => "account_id",
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
