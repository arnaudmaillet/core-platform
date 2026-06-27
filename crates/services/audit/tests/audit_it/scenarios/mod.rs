//! Live scenarios, grouped by concern. Each mints a fresh tenant so its chain
//! partition is unique and the suite runs in parallel against the shared
//! containers.

mod account;
mod adversarial_witness;
mod anchor;
mod auth;
mod chain;
mod idempotency;
mod kms_cipher;
mod moderation;
mod shred;
