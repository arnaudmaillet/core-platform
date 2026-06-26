//! Write-side use cases. Search has exactly one: project an inbound source event
//! and apply the mutation to the index. There is no synchronous write RPC — every
//! ingestion consumer drives [`ProjectionHandler`].

pub mod apply;

pub use apply::{ApplyOutcome, ProjectionHandler};
