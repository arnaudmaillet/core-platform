//! The search service's composition root.
//!
//! [`App::compose`] is *pure* wiring: the index port in, the assembled gRPC handler
//! out — no I/O, so the unit/integration graph and the binary build the exact same
//! handler. [`App::build`] is the I/O variant that constructs the OpenSearch
//! adapter, the ingestion projection handler, and the gRPC content hydrator from
//! config, then defers to `compose`.

use std::sync::Arc;

use tonic::transport::Channel;

use crate::application::command::ProjectionHandler;
use crate::application::port::SearchIndex;
use crate::application::query::{SearchHandler, SuggestHandler};
use crate::config::SearchConfig;
use crate::infrastructure::grpc::SearchServiceHandler;
use crate::infrastructure::hydrate::{GrpcPostHydrator, SourceHydrator};
use crate::infrastructure::index::OpenSearchIndex;

/// A fully-wired search service. Retains the concrete engine adapter (for the
/// liveness probe), the projection handler + hydrator (for the self-spawned
/// ingestion consumers), beside the gRPC handler.
pub struct App {
    pub handler: SearchServiceHandler,
    pub projection: Arc<ProjectionHandler>,
    pub hydrator: Arc<dyn SourceHydrator>,
    pub index: Arc<OpenSearchIndex>,
}

impl App {
    /// Pure composition: build the two query handlers from the index port and wrap
    /// them in the gRPC handler. Drives the unit/integration graph.
    pub fn compose(index: Arc<dyn SearchIndex>) -> SearchServiceHandler {
        let search = Arc::new(SearchHandler::new(Arc::clone(&index)));
        let suggest = Arc::new(SuggestHandler::new(Arc::clone(&index)));
        SearchServiceHandler::new(search, suggest)
    }

    /// Builds the concrete adapter graph from config + backend connections.
    pub async fn build(config: SearchConfig) -> Result<App, Box<dyn std::error::Error>> {
        // The HTTP client carries the configured request timeout (fail-open on a
        // slow engine).
        let index = Arc::new(OpenSearchIndex::from_config(config.opensearch));

        // Best-effort schema bootstrap: a fail-open service must not refuse to boot
        // because the engine is briefly unreachable — indices can be (re)created by
        // ops / the reindex job. Log and continue.
        if let Err(error) = {
            use crate::application::port::IndexAdmin;
            index.ensure_indices().await
        } {
            tracing::warn!(%error, "ensure_indices failed at boot; continuing (fail-open)");
        }

        let index_port: Arc<dyn SearchIndex> = index.clone();
        let projection = Arc::new(ProjectionHandler::new(Arc::clone(&index_port)));

        // Lazy connect: dials `post` on first use, so a cold start does not require
        // the dependency to be up at boot.
        let channel = Channel::from_shared(config.post_endpoint)?.connect_lazy();
        let hydrator: Arc<dyn SourceHydrator> = Arc::new(GrpcPostHydrator::new(channel));

        let handler = App::compose(index_port);
        Ok(App {
            handler,
            projection,
            hydrator,
            index,
        })
    }
}

#[cfg(test)]
mod tests {
    use cqrs::Envelope;
    use tonic::Request;
    use uuid::Uuid;

    use super::*;
    use crate::application::fakes::{Fixture, post_event};
    use crate::domain::SourceEvent;
    use crate::infrastructure::grpc::proto;

    async fn index_post(fx: &Fixture, id: &str, author: &str, caption: &str) {
        let event: SourceEvent = post_event(id, author, caption, 1);
        fx.projection_handler()
            .apply(Envelope::new(Uuid::now_v7(), event), fx.now())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn search_rpc_returns_matching_hits() {
        let fx = Fixture::new();
        index_post(&fx, "post-1", "acct-1", "learning rust").await;
        index_post(&fx, "post-2", "acct-2", "cooking dinner").await;

        let handler = App::compose(fx.index.clone());
        let request = Request::new(proto::SearchRequest {
            query: "rust".into(),
            entity_types: vec![],
            sort: 0,
            page_size: 10,
            page_token: String::new(),
            exclude_author_ids: vec![],
        });
        let resp = handler.search(request).await.unwrap().into_inner();
        assert_eq!(resp.hits.len(), 1);
        assert_eq!(resp.hits[0].id, "post-1");
        assert!(!resp.degraded);
    }

    #[tokio::test]
    async fn search_rpc_rejects_empty_query() {
        let fx = Fixture::new();
        let handler = App::compose(fx.index.clone());
        let request = Request::new(proto::SearchRequest {
            query: "   ".into(),
            entity_types: vec![],
            sort: 0,
            page_size: 10,
            page_token: String::new(),
            exclude_author_ids: vec![],
        });
        let status = handler.search(request).await.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn multi_search_runs_each_query() {
        let fx = Fixture::new();
        index_post(&fx, "post-1", "acct-1", "rust lang").await;
        index_post(&fx, "post-2", "acct-2", "pasta recipe").await;

        let handler = App::compose(fx.index.clone());
        let one = |q: &str| proto::SearchRequest {
            query: q.into(),
            entity_types: vec![],
            sort: 0,
            page_size: 10,
            page_token: String::new(),
            exclude_author_ids: vec![],
        };
        let request = Request::new(proto::MultiSearchRequest {
            searches: vec![one("rust"), one("pasta")],
        });
        let resp = handler.multi_search(request).await.unwrap().into_inner();
        assert_eq!(resp.responses.len(), 2);
        assert_eq!(resp.responses[0].hits[0].id, "post-1");
        assert_eq!(resp.responses[1].hits[0].id, "post-2");
    }
}
