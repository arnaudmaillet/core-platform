// crates/account/src/domain/builders/account_metadata_builder.rs

use crate::domain::entities::AccountMetadata;
use crate::domain::value_objects::AccountRole;
use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

pub struct AccountMetadataBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    role: AccountRole,
    trust_score: i32,
    estimated_ip: Option<String>,
    version: u64,
}

impl AccountMetadataBuilder {
    /// CHEMIN 1 : CRÉATION (Via Use Case)
    pub(crate) fn new(account_id: AccountId, region_code: RegionCode) -> Self {
        Self {
            account_id,
            region_code,
            role: AccountRole::User,
            trust_score: 50,
            estimated_ip: None,
            version: 1,
        }
    }

    /// CHEMIN 2 : RESTAURATION (Via Repository)
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
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
        version: u64,
    ) -> AccountMetadata {
        AccountMetadata::restore(
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
            AggregateMetadata::restore(version),
        )
    }

    // --- SETTERS FLUIDES ---

    pub fn with_role(mut self, role: AccountRole) -> Self {
        self.role = role;
        self
    }

    pub fn with_estimated_ip(mut self, ip: String) -> Self {
        self.estimated_ip = Some(ip);
        self
    }

    pub fn with_optional_estimated_ip(mut self, ip: Option<String>) -> Self {
        self.estimated_ip = ip;
        self
    }

    pub fn with_trust_score(mut self, score: i32) -> Self {
        self.trust_score = score;
        self
    }

    /// Finalise pour une CRÉATION
    pub fn build(self) -> AccountMetadata {
        let now = Utc::now();

        // On centralise l'instanciation via restore même pour le build initial
        AccountMetadata::restore(
            self.account_id,
            self.region_code,
            self.role,
            false, // is_beta_tester
            false, // is_shadowbanned
            self.trust_score,
            None,  // last_moderation_at
            Some(format!(
                "[{}] Account metadata initialized.",
                now.format("%Y-%m-%d %H:%M:%S")
            )),
            self.estimated_ip,
            now,
            AggregateMetadata::new(self.version),
        )
    }
}