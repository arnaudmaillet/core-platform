use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::event::{DomainEvent, SubjectLinked};
use crate::domain::value_object::{AccountId, IdpSubject};

/// The immutable binding of an IdP subject to an internal account.
///
/// This is the one piece of identity data the auth context legitimately owns —
/// it is an authentication concern, not an identity-record concern (the record
/// lives in `account`). Keying on the full [`IdpSubject`] `(issuer, subject)` is
/// what makes IdP migration safe.
///
/// # Invariant 5 — immutable once established
/// A link has no mutating methods: there is no setter for `account_id` or
/// `subject`. Re-pointing a subject to a different account is modelled elsewhere
/// as a *new* link plus an audit trail, never as a mutation of this aggregate.
/// Uniqueness (one link per subject) is enforced at the repository boundary,
/// surfacing as [`AuthError::SubjectAlreadyLinked`](crate::error::AuthError::SubjectAlreadyLinked).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectLink {
    subject: IdpSubject,
    account_id: AccountId,
    linked_at: DateTime<Utc>,
    version: i64,

    #[serde(skip)]
    pending_events: Vec<DomainEvent>,
}

impl SubjectLink {
    /// Establishes a new link on first login. Emits [`SubjectLinked`].
    pub fn establish(
        subject: IdpSubject,
        account_id: AccountId,
        now: DateTime<Utc>,
        correlation_id: Uuid,
    ) -> Self {
        let event = DomainEvent::SubjectLinked(SubjectLinked {
            account_id,
            subject: subject.clone(),
            occurred_at: now,
            correlation_id,
        });
        Self {
            subject,
            account_id,
            linked_at: now,
            version: 0,
            pending_events: vec![event],
        }
    }

    /// Reconstructs from storage (no events emitted).
    pub fn reconstitute(
        subject: IdpSubject,
        account_id: AccountId,
        linked_at: DateTime<Utc>,
        version: i64,
    ) -> Self {
        Self {
            subject,
            account_id,
            linked_at,
            version,
            pending_events: Vec::new(),
        }
    }

    pub fn subject(&self) -> &IdpSubject {
        &self.subject
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn linked_at(&self) -> DateTime<Utc> {
        self.linked_at
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    #[test]
    fn establish_emits_subject_linked() {
        let subject = IdpSubject::new("iss", "sub").unwrap();
        let account = AccountId::from_uuid(Uuid::now_v7());
        let mut link = SubjectLink::establish(subject.clone(), account, t0(), Uuid::now_v7());

        assert_eq!(link.subject(), &subject);
        assert_eq!(link.account_id(), account);
        let events = link.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "auth.subject_linked");
    }

    #[test]
    fn reconstituted_link_emits_nothing() {
        let mut link = SubjectLink::reconstitute(
            IdpSubject::new("iss", "sub").unwrap(),
            AccountId::from_uuid(Uuid::now_v7()),
            t0(),
            3,
        );
        assert!(link.drain_events().is_empty());
        assert_eq!(link.version(), 3);
    }
}
