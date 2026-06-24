//! A `Clone + Send + Sync` boxed [`Service`].
//!
//! [`tower::util::BoxCloneService`] (tower 0.4) erases to a `Send`-only trait object, so a
//! service built on top of it is not `Sync`. The resilient gRPC channel, however, is stored
//! behind an `Arc` in a client adapter that is shared across worker tasks (`Arc<T>: Sync`
//! requires `T: Sync`), so the erased channel must itself be `Sync`.
//!
//! tower 0.5 ships `BoxCloneSyncService` for exactly this, but the workspace is pinned to
//! tower 0.4. This is a minimal port of that type: identical to `BoxCloneService` except the
//! erased trait object (and the inner-service bound) carries `+ Sync`.

use std::{
    fmt,
    task::{Context, Poll},
};

use futures_util::future::BoxFuture;
use tower::Service;
use tower::ServiceExt;

/// A [`Clone`] + [`Send`] + [`Sync`] boxed [`Service`].
///
/// See the [module docs](self) for why the `Sync` bound matters here.
pub struct BoxCloneSyncService<T, U, E>(
    Box<
        dyn CloneService<T, Response = U, Error = E, Future = BoxFuture<'static, Result<U, E>>>
            + Send
            + Sync,
    >,
);

impl<T, U, E> BoxCloneSyncService<T, U, E> {
    /// Erases `inner` into a `Clone + Send + Sync` boxed service.
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        BoxCloneSyncService(Box::new(inner))
    }
}

impl<T, U, E> Service<T> for BoxCloneSyncService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<'static, Result<U, E>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, request: T) -> Self::Future {
        self.0.call(request)
    }
}

impl<T, U, E> Clone for BoxCloneSyncService<T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait CloneService<R>: Service<R> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<R, Response = Self::Response, Error = Self::Error, Future = Self::Future>
            + Send
            + Sync,
    >;
}

impl<R, T> CloneService<R> for T
where
    T: Service<R> + Clone + Send + Sync + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<R, Response = T::Response, Error = T::Error, Future = T::Future>
            + Send
            + Sync,
    > {
        Box::new(self.clone())
    }
}

impl<T, U, E> fmt::Debug for BoxCloneSyncService<T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxCloneSyncService").finish()
    }
}
