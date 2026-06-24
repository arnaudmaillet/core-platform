use std::future::Future;

use ::error::AppError;

use crate::envelope::Envelope;

use super::command::Command;

/// Handles a single command type `C`.
///
/// Each concrete handler is registered with a [`CommandBus`] for exactly
/// one command type. All domain mutation logic lives inside `handle`.
///
/// ## Error contract
///
/// The associated `Error` type must implement [`AppError`]. It will be
/// type-erased and wrapped in [`CqrsError::Handler`] by the bus before
/// surfacing to the caller. Do **not** use [`CqrsError`] as your handler's
/// `Error` type — that is the bus's infrastructure type.
///
/// ## Example
///
/// ```rust
/// use cqrs::{Command, CommandHandler, Envelope};
///
/// struct CreatePostCommand { title: String }
/// impl Command for CreatePostCommand {}
///
/// struct CreatePostHandler { /* db, repo, etc. */ }
///
/// impl CommandHandler<CreatePostCommand> for CreatePostHandler {
///     type Error = PostServiceError;
///
///     async fn handle(
///         &self,
///         envelope: Envelope<CreatePostCommand>,
///     ) -> Result<(), Self::Error> {
///         // domain logic here
///         Ok(())
///     }
/// }
/// ```
pub trait CommandHandler<C: Command>: Send + Sync + 'static {
    type Error: AppError;

    fn handle(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + '_;
}
