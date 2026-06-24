/// Marker trait for all query types.
///
/// A query represents a read-only request that returns data without
/// side-effects. It is named as a question (`GetPostById`, `ListUserFeed`,
/// `FindNearbyPlaces`) and must carry all filtering / pagination parameters
/// the handler needs.
///
/// ## Associated type `Response`
///
/// Every query declares its return type via the `Response` associated type,
/// enabling the [`QueryBus`] to return typed results without casting.
///
/// ## Requirements
///
/// `Send + Sync + 'static` are required for the same reasons as [`Command`]:
/// thread safety and storage in the type-erased registry.
/// `Response` must additionally be `Send + Sync + 'static` so it can be
/// returned across thread boundaries after a `BoxFuture` resolves.
pub trait Query: Send + Sync + 'static {
    type Response: Send + Sync + 'static;
}
