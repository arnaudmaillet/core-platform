use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::SearchIndex;
use crate::domain::{SearchQuery, SearchResults};
use crate::error::SearchError;

/// The query-bus wrapper around a validated domain [`SearchQuery`]. (Validation /
/// proto mapping happens at the edge in Phase 5; by the time it reaches here the
/// `SearchQuery` is already well-formed.)
#[derive(Debug, Clone)]
pub struct RunSearch {
    pub query: SearchQuery,
}

impl Query for RunSearch {
    type Response = SearchResults;
}

pub struct SearchHandler {
    index: Arc<dyn SearchIndex>,
}

impl SearchHandler {
    pub fn new(index: Arc<dyn SearchIndex>) -> Self {
        Self { index }
    }
}

impl QueryHandler<RunSearch> for SearchHandler {
    type Error = SearchError;

    async fn handle(&self, envelope: Envelope<RunSearch>) -> Result<SearchResults, Self::Error> {
        self.index.search(&envelope.payload.query).await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::application::command::ProjectionHandler;
    use crate::application::fakes::{Fixture, post_event};
    use crate::domain::{AuthorId, EntityKind, SortStrategy, SourceEvent};

    async fn index(h: &ProjectionHandler, fx: &Fixture, event: SourceEvent) {
        h.apply(Envelope::new(Uuid::now_v7(), event), fx.now())
            .await
            .unwrap();
    }

    fn run(query: SearchQuery) -> Envelope<RunSearch> {
        Envelope::new(Uuid::now_v7(), RunSearch { query })
    }

    #[tokio::test]
    async fn returns_matching_visible_documents() {
        let fx = Fixture::new();
        let ph = fx.projection_handler();
        index(&ph, &fx, post_event("post-1", "acct-1", "learning rust today", 1)).await;
        index(&ph, &fx, post_event("post-2", "acct-2", "cooking pasta", 1)).await;

        let q = SearchQuery::new("rust", vec![], SortStrategy::Relevance, 10, None, vec![]).unwrap();
        let results = fx.search_handler().handle(run(q)).await.unwrap();

        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].id, "post-1");
        assert!(!results.degraded);
    }

    #[tokio::test]
    async fn excludes_blocked_authors() {
        let fx = Fixture::new();
        let ph = fx.projection_handler();
        index(&ph, &fx, post_event("post-1", "blocked", "rust one", 1)).await;
        index(&ph, &fx, post_event("post-2", "ok", "rust two", 1)).await;

        let q = SearchQuery::new(
            "rust",
            vec![EntityKind::Post],
            SortStrategy::Relevance,
            10,
            None,
            vec![AuthorId::new("blocked").unwrap()],
        )
        .unwrap();
        let results = fx.search_handler().handle(run(q)).await.unwrap();

        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].id, "post-2");
    }

    #[tokio::test]
    async fn hidden_documents_are_not_returned() {
        let fx = Fixture::new();
        let ph = fx.projection_handler();
        index(&ph, &fx, post_event("post-1", "acct-1", "rust hidden", 1)).await;
        fx.index.force_hidden(EntityKind::Post, "post-1");

        let q = SearchQuery::new("rust", vec![], SortStrategy::Relevance, 10, None, vec![]).unwrap();
        let results = fx.search_handler().handle(run(q)).await.unwrap();

        assert!(results.hits.is_empty());
    }
}
