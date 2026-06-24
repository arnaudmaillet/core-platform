pub mod task_local;

pub use task_local::{AnyPrincipal, current_principal, inject_into_span, with_principal};

#[cfg(feature = "cqrs-integration")]
pub use task_local::inject_into_envelope;
