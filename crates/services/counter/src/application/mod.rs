//! The counter-analytics application layer — use-case orchestration over the
//! domain and the ports.
//!
//! ## Handler shape
//! The write-side use cases ([`command::DeltaFlusher`], [`command::PopularityPublisher`])
//! are plain application-service structs (not `cqrs::CommandHandler`s): the flush
//! returns a rich [`command::FlushReport`], and the popularity publisher is a
//! slow-loop side effect — neither fits the command-bus request/response shape.
//! The read-side use cases ([`query::BatchGetHandler`], [`query::TrendingHandler`],
//! [`query::TimeSeriesHandler`]) implement [`cqrs::QueryHandler`] and ride the
//! query bus.
//!
//! ## Ports
//! Every external dependency is an `async_trait` port in [`port`], injected as an
//! `Arc<dyn …>` at the composition root, so the handlers never name a concrete
//! adapter. In-memory fakes back the unit tests.

pub mod command;
pub mod port;
pub mod query;

#[cfg(test)]
pub mod fakes;

pub use command::{DeltaFlusher, FlushReport, PopularityPublisher};
pub use port::{CounterLedger, CounterStore, FlushOutcome, SignalPublisher, TimeSeriesStore};
pub use query::{
    BatchGetHandler, RunBatchGet, RunTimeSeries, RunTrending, TimeSeriesHandler, TrendingHandler,
};
