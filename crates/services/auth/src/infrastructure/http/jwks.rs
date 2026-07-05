//! The well-known JWKS endpoint.
//!
//! Serves the process's fixed key ring (serialized once at `App::build`) at
//! `GET /.well-known/jwks.json`. The ring never changes within a process —
//! key rotation is a redeploy — so the body is a static string and the
//! handler is allocation-free per request. Listener address comes from
//! `AUTH_JWKS_HTTP_ADDR` (default `0.0.0.0:8081`); the port is cluster-internal
//! (NetworkPolicy scoped to the verifying services + kubelet probes).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

/// The well-known path, shared with every downstream verifier's config.
pub const JWKS_PATH: &str = "/.well-known/jwks.json";

/// Builds the single-route JWKS router over the pre-serialized document.
pub fn router(jwks_json: String) -> Router {
    let body: Arc<str> = jwks_json.into();
    Router::new().route(JWKS_PATH, get(serve_jwks)).with_state(body)
}

async fn serve_jwks(State(body): State<Arc<str>>) -> impl IntoResponse {
    // Verifiers poll on their own refresh interval (auth-context defaults);
    // a short shared-cache TTL keeps any future proxy layer from pinning a
    // stale ring across a redeploy longer than a probe cycle.
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=60"),
        ],
        body.to_string(),
    )
}

/// Binds and serves the JWKS listener until the process exits. Spawned as a
/// background task by the runtime adapter; a bind failure is returned to the
/// caller (fail-fast at boot — a fleet where verifiers cannot fetch keys is
/// fail-closed everywhere downstream, better to crash-loop visibly here).
pub async fn serve(addr: SocketAddr, jwks_json: String) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, path = JWKS_PATH, "auth.jwks http listener up");
    axum::serve(listener, router(jwks_json)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_the_document_at_the_well_known_path() {
        let doc = r#"{"keys":[{"kty":"EC","kid":"k1"}]}"#;
        let response = router(doc.to_owned())
            .oneshot(Request::get(JWKS_PATH).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/json"
        );
        let bytes = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&bytes[..], doc.as_bytes());
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let response = router("{}".to_owned())
            .oneshot(Request::get("/jwks").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
