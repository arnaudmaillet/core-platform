//! Token cryptography adapter: ES256 edge tokens + opaque refresh handles.
//!
//! ES256 (rather than RS256) keeps tokens compact and verification cheap on the
//! edge hot path; the format is swappable for PASETO v4 behind the same
//! [`TokenMinter`](crate::application::port::TokenMinter) port without touching
//! the handlers.

pub mod es256_token_minter;

pub use es256_token_minter::{Es256TokenMinter, EsKeyMaterial, EsVerifyingKey, Jwk, Jwks};
