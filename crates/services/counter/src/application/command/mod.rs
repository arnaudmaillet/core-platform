//! Write-side use cases. Both are plain application-service structs (not
//! `cqrs::CommandHandler`s): the ingestion path returns a rich [`FlushReport`], and
//! the popularity path is a slow-loop side effect — neither fits the
//! command-bus request/response shape.

pub mod flush;
pub mod popularity;
pub mod reconcile;

pub use flush::{DeltaFlusher, FlushReport};
pub use popularity::PopularityPublisher;
pub use reconcile::{ReconcileOutcome, Reconciler};
