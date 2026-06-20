use std::any::Any;
use std::future::Future;
use std::sync::Arc;

use crate::{CurrentPrincipal, Permission, PrincipalId};

// ── Object-safe principal facade ─────────────────────────────────────────────

/// Object-safe view over a [`CurrentPrincipal<C>`] for type-erased task-local
/// storage.
///
/// Exposes only the provider-agnostic fields so telemetry middleware and
/// envelope injection helpers can operate without knowing the concrete `C` type.
/// Use [`as_any`] to downcast back to `CurrentPrincipal<C>` when the concrete
/// type is available in scope.
///
/// [`as_any`]: AnyPrincipal::as_any
pub trait AnyPrincipal: Send + Sync {
    fn user_id(&self) -> &PrincipalId;
    fn tenant_id(&self) -> Option<&str>;
    fn permissions(&self) -> &[Permission];

    /// Enables downcasting to the concrete `CurrentPrincipal<C>` type when
    /// the caller knows `C` at the point of use.
    fn as_any(&self) -> &dyn Any;
}

impl<C: Send + Sync + 'static> AnyPrincipal for CurrentPrincipal<C> {
    fn user_id(&self) -> &PrincipalId {
        &self.user_id
    }

    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }

    fn permissions(&self) -> &[Permission] {
        &self.permissions
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ── Task-local storage ────────────────────────────────────────────────────────

tokio::task_local! {
    /// The authenticated principal bound to the current async task.
    ///
    /// Set by [`with_principal`]; read by [`current_principal`].
    /// Absent outside of a [`with_principal`] scope — `try_with` returns `None`.
    static CURRENT_PRINCIPAL: Arc<dyn AnyPrincipal>;
}

// ── Public lifecycle API ──────────────────────────────────────────────────────

/// Runs `future` with `principal` bound to the task-local identity slot.
///
/// All code within `future` — including calls across `.await` points on the
/// same logical task — can retrieve the principal via [`current_principal`]
/// without passing it through function signatures.
///
/// Nesting is supported: an inner [`with_principal`] call shadows the outer
/// one for its subtree, restoring the original on exit.
///
/// # Example
///
/// ```rust,ignore
/// let principal = Arc::new(CurrentPrincipal { ... });
///
/// with_principal(principal, async {
///     // current_principal() is Some(...) here
///     some_service_call().await;
/// }).await;
///
/// // current_principal() is None again here
/// ```
pub fn with_principal<P, Fut>(principal: Arc<P>, future: Fut) -> impl Future<Output = Fut::Output>
where
    P: AnyPrincipal + 'static,
    Fut: Future,
{
    CURRENT_PRINCIPAL.scope(principal as Arc<dyn AnyPrincipal>, future)
}

/// Returns the principal currently bound to this task, or `None` if called
/// outside a [`with_principal`] scope.
pub fn current_principal() -> Option<Arc<dyn AnyPrincipal>> {
    CURRENT_PRINCIPAL.try_with(Arc::clone).ok()
}

// ── Integration helpers ───────────────────────────────────────────────────────

/// Enriches the *current* `tracing` span with the identity fields of the
/// task-local principal.
///
/// ## Fields written
///
/// | Span field             | Source                        |
/// |------------------------|-------------------------------|
/// | `principal.user_id`    | [`PrincipalId`] string        |
/// | `principal.tenant_id`  | Tenant string (if present)    |
///
/// ## Pre-requisites
///
/// The surrounding span must declare these fields as `Empty` for the record
/// call to take effect:
///
/// ```rust,ignore
/// #[tracing::instrument(fields(
///     principal.user_id  = tracing::field::Empty,
///     principal.tenant_id = tracing::field::Empty,
/// ))]
/// async fn my_handler(...) {
///     inject_into_span();
///     // ...
/// }
/// ```
///
/// This is a no-op when called outside a [`with_principal`] scope.
pub fn inject_into_span() {
    if let Some(p) = current_principal() {
        let span = tracing::Span::current();
        span.record("principal.user_id", p.user_id().as_str());
        if let Some(tid) = p.tenant_id() {
            span.record("principal.tenant_id", tid);
        }
    }
}

/// Injects the task-local principal's identity into an [`Envelope`]'s
/// `metadata` map.
///
/// ## Keys written
///
/// | Metadata key           | Value                         |
/// |------------------------|-------------------------------|
/// | `principal.user_id`    | [`PrincipalId`] string        |
/// | `principal.tenant_id`  | Tenant string (if present)    |
///
/// This is a no-op when called outside a [`with_principal`] scope.
///
/// Requires the `cqrs-integration` Cargo feature.
///
/// [`Envelope`]: cqrs::Envelope
#[cfg(feature = "cqrs-integration")]
pub fn inject_into_envelope<T>(envelope: &mut cqrs::Envelope<T>) {
    if let Some(p) = current_principal() {
        envelope.metadata.insert(
            "principal.user_id".to_owned(),
            p.user_id().to_string(),
        );
        if let Some(tid) = p.tenant_id() {
            envelope
                .metadata
                .insert("principal.tenant_id".to_owned(), tid.to_owned());
        }
    }
}
