//! The moderation bounded context's domain layer — pure, free of I/O.
//!
//! It owns the *integrity decision and its consequences*: the [`Case`] review
//! lifecycle, the append-only [`Decision`] ledger (the legal evidence record),
//! the [`EnforcementAction`] with its monotonic per-subject version, the
//! [`PenaltyLedger`] graduated-enforcement engine, the [`Appeal`] lifecycle, and
//! the [`Report`] intake record. Content bytes live in the content services;
//! account lifecycle lives in `account`; ML inference lives in the classifier
//! services — none of those are modelled here.
//!
//! ## Clock injection
//! Every time-dependent invariant (strike decay, enforcement expiry, appeal
//! windows) takes `now: DateTime<Utc>` as a parameter rather than reading the
//! wall clock, so the state machines are deterministically unit-testable. The
//! application layer supplies the clock.
//!
//! [`Case`]: aggregate::Case
//! [`Decision`]: aggregate::Decision
//! [`EnforcementAction`]: aggregate::EnforcementAction
//! [`PenaltyLedger`]: aggregate::PenaltyLedger
//! [`Appeal`]: aggregate::Appeal
//! [`Report`]: aggregate::Report

pub mod aggregate;
pub mod event;
pub mod value_object;
