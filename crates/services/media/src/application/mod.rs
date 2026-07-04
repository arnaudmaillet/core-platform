//! The media application layer — use-case orchestration over the domain and ports.
//!
//! ## Handler shape
//! Like `auth`/`moderation`, the write use-cases are explicit application-service
//! handlers rather than `cqrs::CommandHandler`: they return rich outcomes (a
//! prepared upload, a pipeline result) that a `Result<(), E>` command cannot
//! carry. Each takes a [`cqrs::Envelope`] (so the `correlation_id` threads
//! through) and an injected `now` for deterministic tests. The genuinely
//! read-only use-cases — `GetAsset`, `ResolveDelivery` — implement
//! [`cqrs::QueryHandler`] and ride the query bus.
//!
//! ## The two planes, as handlers
//! * **Plane A (sync, light):** [`IssueUploadTicketHandler`] brokers a pre-signed
//!   upload; [`CommitUploadHandler`] finalizes it (probe → validate → emit
//!   `AssetUploaded`). Neither touches a byte of the payload.
//! * **Plane B (async, heavy):** [`ProcessAssetHandler`] runs the transformation
//!   pipeline (scan → screen → derive → ready), driven off the finalize event by
//!   a worker (Phase 5). [`ApplyModerationHandler`] applies takedowns/restores.
//! * **Compliance:** [`DeleteAssetHandler`] honors the legal hold (erasure is
//!   refused while held) and purges bytes + CDN on a real delete.
//!
//! ## Ports
//! Every external dependency is an `async_trait` port in [`port`], injected as an
//! `Arc<dyn …>` at the composition root, so handlers never name a concrete
//! adapter. In-memory fakes of every port back the unit tests.
//!
//! ## Durable-first ordering
//! Handlers persist to the metadata SoR *first*, then publish the lifecycle event.
//! The durable write is the source of truth; the event is the decoupling feed.
//!
//! [`IssueUploadTicketHandler`]: command::IssueUploadTicketHandler
//! [`CommitUploadHandler`]: command::CommitUploadHandler
//! [`ProcessAssetHandler`]: command::ProcessAssetHandler
//! [`ApplyModerationHandler`]: command::ApplyModerationHandler
//! [`DeleteAssetHandler`]: command::DeleteAssetHandler

pub mod command;
pub mod policy;
pub mod port;
pub mod query;

#[cfg(test)]
pub mod fakes;

pub use policy::MediaPolicy;
