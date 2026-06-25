//! The auth application layer — use-case orchestration over the domain and ports.
//!
//! ## Why not every use-case is a `cqrs::CommandHandler`
//! `cqrs::CommandHandler` returns `Result<(), Error>`: a command mutates and
//! yields nothing. Auth's `Login`/`Refresh`/`Logout`/`LogoutAllSessions` must
//! return data (tokens, a new generation), so they are modelled as explicit
//! application-service handlers with rich return types. The genuinely read-only
//! `Introspect`/`ListSessions` implement [`cqrs::QueryHandler`] and ride the
//! query bus like the rest of the fleet. All handlers take a [`cqrs::Envelope`]
//! so the `correlation_id` threads into the domain events they emit.
//!
//! ## Ports
//! Every external dependency is an `async_trait` port in [`port`], injected as an
//! `Arc<dyn …>` at the composition root. The handlers never name a concrete
//! adapter — that is what keeps the layer (and the domain) IdP- and
//! storage-agnostic. In-memory fakes of every port back the unit tests.

pub mod command;
pub mod policy;
pub mod port;
pub mod query;

#[cfg(test)]
pub mod fakes;

pub use policy::SessionPolicy;

use validate_core::Validate;

use crate::error::AuthError;

/// Runs a payload's self-validation, mapping field violations to [`AuthError`].
///
/// Handlers call this at the top of `handle`, since the auth use-cases bypass the
/// command bus (and thus the bus's `ValidationLayer`).
pub(crate) fn ensure_valid<T: Validate>(payload: &T) -> Result<(), AuthError> {
    match payload.validate() {
        Ok(()) => Ok(()),
        Err(violations) => Err(validation::ValidationError::new(violations).into()),
    }
}
