use serde::{Deserialize, Serialize};

/// The compliance category of an audit event — the primary classifier that drives
/// retention policy and chain partitioning.
///
/// These are pure domain enums, deliberately independent of the generated
/// `audit-api` proto enums; the mapping between the two lives in the
/// infrastructure tier (so a wire-format change never reaches the domain).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventCategory {
    Authentication,
    Authorization,
    Moderation,
    Consent,
    DataAccess,
    DataExport,
    DataErasure,
    PrivilegedAction,
    Retention,
}

impl EventCategory {
    /// Whether events in this category process personal data and therefore MUST
    /// declare a GDPR Art. 6 lawful basis (enforced at construction → `AUD-1002`).
    ///
    /// Authentication, authorization and the privileged/retention machinery are
    /// system bookkeeping that may legitimately carry no subject; the PII-touching
    /// categories (consent, data access/export/erasure, moderation of a subject)
    /// must be self-describing for an accountability review.
    pub fn requires_lawful_basis(self) -> bool {
        matches!(
            self,
            EventCategory::Moderation
                | EventCategory::Consent
                | EventCategory::DataAccess
                | EventCategory::DataExport
                | EventCategory::DataErasure
        )
    }

    /// A stable byte discriminant for canonical hashing. Explicit (not the enum's
    /// memory layout) so the hash is stable across compiler/version changes — the
    /// chain must verify in five years.
    pub fn hash_tag(self) -> u8 {
        match self {
            EventCategory::Authentication => 1,
            EventCategory::Authorization => 2,
            EventCategory::Moderation => 3,
            EventCategory::Consent => 4,
            EventCategory::DataAccess => 5,
            EventCategory::DataExport => 6,
            EventCategory::DataErasure => 7,
            EventCategory::PrivilegedAction => 8,
            EventCategory::Retention => 9,
        }
    }
}

/// The kind of principal that performed an action. Classifies human vs machine
/// activity for reporting; identities themselves are pseudonymous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActorType {
    User,
    Admin,
    Service,
    System,
}

impl ActorType {
    pub fn hash_tag(self) -> u8 {
        match self {
            ActorType::User => 1,
            ActorType::Admin => 2,
            ActorType::Service => 3,
            ActorType::System => 4,
        }
    }
}

/// The result of the audited action — the "what happened" half of the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    Permitted,
    Denied,
    Executed,
    Failed,
}

impl Outcome {
    pub fn hash_tag(self) -> u8 {
        match self {
            Outcome::Permitted => 1,
            Outcome::Denied => 2,
            Outcome::Executed => 3,
            Outcome::Failed => 4,
        }
    }
}

/// The GDPR Art. 6 lawful basis under which personal-data processing was carried
/// out. `Unspecified` is the absent state — invalid on a category that
/// [`EventCategory::requires_lawful_basis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LawfulBasis {
    Unspecified,
    Consent,
    Contract,
    LegalObligation,
    VitalInterests,
    PublicTask,
    LegitimateInterests,
}

impl LawfulBasis {
    pub fn is_specified(self) -> bool {
        !matches!(self, LawfulBasis::Unspecified)
    }

    pub fn hash_tag(self) -> u8 {
        match self {
            LawfulBasis::Unspecified => 0,
            LawfulBasis::Consent => 1,
            LawfulBasis::Contract => 2,
            LawfulBasis::LegalObligation => 3,
            LawfulBasis::VitalInterests => 4,
            LawfulBasis::PublicTask => 5,
            LawfulBasis::LegitimateInterests => 6,
        }
    }
}

/// The narrow, enumerated set of actions that must be recorded synchronously and
/// fail closed. LOCKED scope: break-glass access + legal-hold lifecycle. The set
/// is intentionally small — every member couples a business flow to audit liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivilegedActionType {
    BreakGlassAccess,
    LegalHoldPlace,
    LegalHoldRelease,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pii_categories_require_lawful_basis() {
        assert!(EventCategory::Consent.requires_lawful_basis());
        assert!(EventCategory::DataErasure.requires_lawful_basis());
        assert!(EventCategory::Moderation.requires_lawful_basis());
        // System bookkeeping does not.
        assert!(!EventCategory::Authentication.requires_lawful_basis());
        assert!(!EventCategory::PrivilegedAction.requires_lawful_basis());
    }

    #[test]
    fn hash_tags_are_distinct_and_explicit() {
        let cats = [
            EventCategory::Authentication,
            EventCategory::Authorization,
            EventCategory::Moderation,
            EventCategory::Consent,
            EventCategory::DataAccess,
            EventCategory::DataExport,
            EventCategory::DataErasure,
            EventCategory::PrivilegedAction,
            EventCategory::Retention,
        ];
        let mut tags: Vec<u8> = cats.iter().map(|c| c.hash_tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), cats.len(), "category hash tags must be unique");
    }

    #[test]
    fn lawful_basis_specified_check() {
        assert!(!LawfulBasis::Unspecified.is_specified());
        assert!(LawfulBasis::LegalObligation.is_specified());
    }
}
