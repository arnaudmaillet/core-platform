//! The media bounded context's domain layer — pure, free of I/O.
//!
//! It owns the *asset and its processing lifecycle*: the [`Asset`] aggregate (the
//! state machine PENDING → UPLOADED → PROCESSING → READY / FAILED / QUARANTINED /
//! DELETED), its [`Rendition`] catalog, the content-addressed [`StorageKey`]
//! scheme, the [`UploadTicket`] reservation, and the value objects that make an
//! invalid asset unrepresentable. The bytes themselves live in object storage; the
//! CDN owns delivery fan-out; `moderation` owns the integrity decision — none of
//! those are modelled here. This layer holds only the *truth about* the bytes.
//!
//! ## Clock injection
//! Every time-dependent transition takes `now: DateTime<Utc>` as a parameter
//! rather than reading the wall clock, so the state machine is deterministically
//! unit-testable. The application layer supplies the clock.
//!
//! [`Asset`]: aggregate::Asset
//! [`Rendition`]: aggregate::Rendition
//! [`StorageKey`]: value_object::StorageKey
//! [`UploadTicket`]: value_object::UploadTicket

pub mod aggregate;
pub mod event;
pub mod value_object;
