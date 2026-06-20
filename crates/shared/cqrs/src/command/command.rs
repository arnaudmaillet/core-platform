/// Marker trait for all command types.
///
/// A command represents a user's intent to change system state. It is named
/// in the imperative mood (`CreatePost`, `FollowUser`, `PublishComment`) and
/// carries all data the handler needs to perform the operation.
///
/// Commands are dispatched **exactly once** to a single registered handler.
/// For fan-out or event broadcasting, use the event/messaging layer instead.
///
/// ## Requirements
///
/// `Send + Sync + 'static` are required so commands can be safely moved
/// across thread and task boundaries inside a multi-threaded async runtime
/// and stored in the type-erased registry without lifetime restrictions.
pub trait Command: Send + Sync + 'static {}
