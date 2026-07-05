//! Plain-HTTP surfaces of the auth service.
//!
//! The fleet mesh is gRPC, but two things are HTTP by contract: downstream
//! verifiers (`auth-context` in realtime/audit) fetch the JWKS from a
//! well-known URL, and that fetch must work with any off-the-shelf JWKS
//! client. This module hosts that listener; it is spawned by the runtime
//! adapter ([`crate::service::AuthService`]) next to the gRPC server.

pub mod jwks;
