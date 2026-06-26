use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::SearchIndex;
use crate::domain::{SuggestQuery, Suggestions};
use crate::error::SearchError;

/// The query-bus wrapper around a validated [`SuggestQuery`].
#[derive(Debug, Clone)]
pub struct RunSuggest {
    pub query: SuggestQuery,
}

impl Query for RunSuggest {
    type Response = Suggestions;
}

pub struct SuggestHandler {
    index: Arc<dyn SearchIndex>,
}

impl SuggestHandler {
    pub fn new(index: Arc<dyn SearchIndex>) -> Self {
        Self { index }
    }
}

impl QueryHandler<RunSuggest> for SuggestHandler {
    type Error = SearchError;

    async fn handle(&self, envelope: Envelope<RunSuggest>) -> Result<Suggestions, Self::Error> {
        self.index.suggest(&envelope.payload.query).await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::application::fakes::{Fixture, profile_event};
    use crate::domain::SourceEvent;

    #[tokio::test]
    async fn completes_a_handle_prefix() {
        let fx = Fixture::new();
        let ph = fx.projection_handler();
        for (id, handle) in [("p1", "alice"), ("p2", "alan"), ("p3", "bob")] {
            let event: SourceEvent = profile_event(id, handle, 1);
            ph.apply(Envelope::new(Uuid::now_v7(), event), fx.now())
                .await
                .unwrap();
        }

        let q = SuggestQuery::new("al", vec![], 10).unwrap();
        let out = fx
            .suggest_handler()
            .handle(Envelope::new(Uuid::now_v7(), RunSuggest { query: q }))
            .await
            .unwrap();

        let texts: Vec<&str> = out.suggestions.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(out.suggestions.len(), 2);
        assert!(texts.contains(&"alice"));
        assert!(texts.contains(&"alan"));
    }
}
