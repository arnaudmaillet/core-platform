// crates/account/src/domain/builders/account_metadata_builder.rs

use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use crate::domain::entities::AccountMetadata;
use crate::domain::value_objects::AccountRole;

pub struct AccountMetadataBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    role: AccountRole,
    trust_score: i32,
    estimated_ip: Option<String>,
    // La version est stockée ici pour être passée à l'agrégat final
    version: i32,
}

impl AccountMetadataBuilder {
    /// CHEMIN 1 : CRÉATION (Via Use Case)
    pub fn new(account_id: AccountId, region_code: RegionCode) -> Self {
        Self {
            account_id,
            region_code,
            role: AccountRole::User,
            trust_score: 0,
            estimated_ip: None,
            version: 1, // État initial de tout nouvel agrégat
        }
    }

    /// CHEMIN 2 : RESTAURATION (Via Repository)
    /// Injection directe de la version provenant de PostgreSQL
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
        version: i32,
    ) -> AccountMetadata {
        AccountMetadata {
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
            metadata: AggregateMetadata::restore(version),
        }
    }

    // --- SETTERS (Chemin Création) ---

    pub fn with_estimated_ip(mut self, ip: String) -> Self {
        self.estimated_ip = Some(ip);
        self
    }

    pub fn with_role(mut self, role: AccountRole) -> Self {
        self.role = role;
        self
    }

    /// Finalise pour une CRÉATION
    pub fn build(self) -> AccountMetadata {
        let now = Utc::now();
        AccountMetadata {
            account_id: self.account_id,
            region_code: self.region_code,
            role: self.role,
            is_beta_tester: false,
            is_shadowbanned: false,
            trust_score: self.trust_score,
            last_moderation_at: None,
            moderation_notes: Some(format!(
                "[{}] Account metadata initialized.",
                now.format("%Y-%m-%d %H:%M:%S")
            )),
            estimated_ip: self.estimated_ip,
            updated_at: now,
            // On initialise un nouveau Metadata technique (Version 1)
            metadata: AggregateMetadata::new(self.version),
        }
    }
}