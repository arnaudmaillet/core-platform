use std::future::Future;

use crate::envelope::Envelope;
use crate::error::CqrsError;

use super::command::Command;

/// The single entry point for command dispatch.
///
/// Callers never reference handlers directly — they go through the bus,
/// which runs the full middleware pipeline before invoking the handler.
///
/// ## Error semantics
///
/// | Variant                       | Meaning                                              |
/// |-------------------------------|------------------------------------------------------|
/// | [`CqrsError::HandlerNotFound`]| No handler was registered for `C`. Programming error.|
/// | [`CqrsError::Handler`]        | The handler returned an error. Original [`AppError`] metadata is preserved. |
///
/// ## Object safety
///
/// `CommandBus` is **not** object-safe: the `dispatch` method is generic over
/// `C`. Use concrete types or a newtype wrapper at the composition root if
/// you need to store heterogeneous buses behind a single pointer.
pub trait CommandBus: Send + Sync {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_;
}
