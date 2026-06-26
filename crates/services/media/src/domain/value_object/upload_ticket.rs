use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{AssetId, StorageKey, UploadConstraints};
use crate::error::MediaError;

/// The domain half of a pre-signed upload reservation: *where* the bytes go (the
/// staging [`StorageKey`]), *what* is allowed (the [`UploadConstraints`]), and
/// *until when* (`expires_at`). The infrastructure layer turns this into an actual
/// pre-signed object-store URL — the URL itself is not modelled here because the
/// domain neither mints nor holds bytes or signed URLs. This keeps the reservation
/// policy pure and testable while the signing stays at the edge of the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadTicket {
    asset_id: AssetId,
    storage_key: StorageKey,
    constraints: UploadConstraints,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl UploadTicket {
    /// Issues a ticket valid for `ttl` from `now`. A non-positive `ttl` is a
    /// programming error and is rejected.
    pub fn issue(
        asset_id: AssetId,
        constraints: UploadConstraints,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<Self, MediaError> {
        if ttl <= Duration::zero() {
            return Err(MediaError::DomainViolation {
                field: "upload_ticket.ttl".into(),
                message: "ticket TTL must be positive".into(),
            });
        }
        Ok(Self {
            storage_key: StorageKey::staging(asset_id),
            asset_id,
            constraints,
            issued_at: now,
            expires_at: now + ttl,
        })
    }

    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    pub fn storage_key(&self) -> &StorageKey {
        &self.storage_key
    }

    pub fn constraints(&self) -> &UploadConstraints {
        &self.constraints
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Whether the ticket is still usable at `now`. A finalize against an expired
    /// ticket is rejected with `UploadTicketExpired` (MED-1004) by the caller.
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-26T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn ticket(ttl: Duration) -> Result<UploadTicket, MediaError> {
        UploadTicket::issue(
            AssetId::from_uuid(Uuid::from_u128(1)),
            UploadConstraints::for_kind(crate::domain::value_object::MediaKind::Avatar),
            ttl,
            t0(),
        )
    }

    #[test]
    fn issued_ticket_is_valid_until_expiry() {
        let t = ticket(Duration::minutes(15)).unwrap();
        assert_eq!(t.storage_key().as_str(), format!("uploads/{}", t.asset_id()));
        assert!(t.is_valid_at(t0()));
        assert!(t.is_valid_at(t0() + Duration::minutes(14)));
        assert!(!t.is_valid_at(t0() + Duration::minutes(15)));
        assert!(!t.is_valid_at(t0() + Duration::minutes(20)));
    }

    #[test]
    fn non_positive_ttl_is_rejected() {
        assert!(matches!(
            ticket(Duration::zero()).unwrap_err(),
            MediaError::DomainViolation { .. }
        ));
        assert!(ticket(Duration::seconds(-1)).is_err());
    }
}
