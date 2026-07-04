use std::future::Future;
use std::sync::Arc;

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

/// Forwarding impl so a single registered bus can be shared by reference-counted
/// pointer — the composition root keeps one `Arc<InMemoryQueryBus>` and hands
/// clones to both the gRPC handler and (in tests) the driving harness, without
/// re-registering handlers or wrapping in a newtype.
impl<B: QueryBus> QueryBus for Arc<B> {
    fn dispatch<Q: Query>(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, CqrsError>> + Send + '_ {
        (**self).dispatch(envelope)
    }
}
