use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use error::AppError;

use crate::application::port::CounterStore;
use crate::domain::{TrendingItem, TrendingQuery};
use crate::error::CounterError;

/// The query-bus wrapper around a validated [`TrendingQuery`].
#[derive(Debug, Clone)]
pub struct RunTrending {
    pub query: TrendingQuery,
}

impl Query for RunTrending {
    type Response = Vec<TrendingItem>;
}

/// Serves `GetTrending` from the hot tier's Count-Min-Sketch + heap. Fails **open**
/// to an empty ranking on a transient store outage.
pub struct TrendingHandler {
    hot: Arc<dyn CounterStore>,
}

impl TrendingHandler {
    pub fn new(hot: Arc<dyn CounterStore>) -> Self {
        Self { hot }
    }
}

impl QueryHandler<RunTrending> for TrendingHandler {
    type Error = CounterError;

    async fn handle(&self, envelope: Envelope<RunTrending>) -> Result<Vec<TrendingItem>, Self::Error> {
        let q = envelope.payload.query;
        match self
            .hot
            .top_k(q.scope, q.scope_key.as_deref(), q.metric, q.limit)
            .await
        {
            Ok(items) => Ok(items),
            Err(e) if e.is_retryable() => Ok(Vec::new()), // degrade to empty
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{EntityId, EntityKind, EntityRef, Metric, TrendingScope};

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    fn run(query: TrendingQuery) -> Envelope<RunTrending> {
        Envelope::new(Uuid::now_v7(), RunTrending { query })
    }

    #[tokio::test]
    async fn ranks_by_score_descending() {
        let fx = Fixture::new();
        fx.hot.seed_trending(Metric::View, &post("low"), 10);
        fx.hot.seed_trending(Metric::View, &post("high"), 100);
        fx.hot.seed_trending(Metric::View, &post("mid"), 50);

        let q = TrendingQuery::new(TrendingScope::Global, None, Metric::View, 2).unwrap();
        let items = fx.trending_handler().handle(run(q)).await.unwrap();

        assert_eq!(items.len(), 2); // limited
        assert_eq!(items[0].entity.id.as_str(), "high");
        assert_eq!(items[0].rank, 0);
        assert_eq!(items[1].entity.id.as_str(), "mid");
        assert_eq!(items[1].rank, 1);
    }

    #[tokio::test]
    async fn fails_open_to_empty() {
        let fx = Fixture::new();
        fx.hot.seed_trending(Metric::View, &post("p1"), 10);
        fx.hot.set_unavailable(true);

        let q = TrendingQuery::new(TrendingScope::Global, None, Metric::View, 10).unwrap();
        let items = fx.trending_handler().handle(run(q)).await.unwrap();
        assert!(items.is_empty());
    }
}
