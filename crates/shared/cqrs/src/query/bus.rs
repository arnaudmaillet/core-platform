use std::future::Future;

use crate::envelope::Envelope;
use crate::error::CqrsError;

use super::query::Query;

/// The single entry point for query dispatch.
///
/// Symmetric to [`CommandBus`] but returns the query's associated
/// `Response` type. Not object-safe for the same reason: `dispatch` is
/// generic over `Q`.
///
/// ## Error semantics
///
/// Same variants as [`CommandBus`]: `HandlerNotFound` (programming error)
/// or `Handler` (domain error from the registered handler).
pub trait QueryBus: Send + Sync {
    fn dispatch<Q: Query>(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, CqrsError>> + Send + '_;
}
