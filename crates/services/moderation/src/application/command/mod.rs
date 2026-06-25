//! Write use-cases. Like `auth`, these are explicit application-service handlers
//! (not `cqrs::CommandHandler`) because they return rich outputs — a decision, an
//! enforcement, a screen verdict — that a `Result<(), E>` command cannot carry.
//! Each takes a [`cqrs::Envelope`] so the `correlation_id` threads into the domain
//! events it emits, and an injected `now` for deterministic tests.

pub mod appeal;
pub mod decide_case;
pub mod ingest;
pub mod screen;

pub use appeal::{
    FileAppealCommand, FileAppealHandler, ResolveAppealCommand, ResolveAppealHandler,
    ResolveAppealOutcome,
};
pub use decide_case::{
    AssignCaseCommand, AssignCaseHandler, DecideCaseCommand, DecideCaseHandler, DecideOutcome,
    OpenCaseCommand, OpenCaseHandler, OpenedCase,
};
pub use ingest::{
    IngestReportCommand, IngestReportHandler, IngestSignalCommand, IngestSignalHandler,
};
pub use screen::{ScreenCommand, ScreenHandler, ScreenOutcome, ScreenVerdict};
