//! The auth bounded context's domain layer — pure, free of I/O.
//!
//! It owns the **authentication act and its lifecycle**: the [`Session`] aggregate
//! (issue / mint / extend / revoke / expire), the [`RefreshToken`] rotation
//! lineage with reuse-detection, and the immutable [`SubjectLink`] that ties an
//! IdP subject to an internal account. Identity records live in the `account`
//! service; credentials live in the IdP — neither is modelled here.
//!
//! ## Clock injection
//! Several invariants are time-based (token/session expiry, refresh-token TTL).
//! To keep the state machine deterministically unit-testable, time-dependent
//! methods take `now: DateTime<Utc>` as a parameter rather than reading the wall
//! clock internally. The application layer supplies the clock.
//!
//! [`Session`]: aggregate::Session
//! [`RefreshToken`]: aggregate::RefreshToken
//! [`SubjectLink`]: aggregate::SubjectLink

pub mod aggregate;
pub mod event;
pub mod value_object;
