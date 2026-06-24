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
/// - `Validate` — every command must be self-validating. Commands that carry
///   no user-supplied data rely on the default no-op impl provided by the
///   trait. This supertrait is what lets the `ValidationLayer` in the
///   `validation` crate call `validate()` generically on any `C: Command`
///   with zero dynamic-dispatch overhead.
/// - `Send + Sync + 'static` — required so commands can be moved across
///   thread and task boundaries and stored in the type-erased registry.
pub trait Command: validate_core::Validate + Send + Sync + 'static {}
