use serde::{Deserialize, Serialize};

/// Result ordering strategy for a query. Default is relevance (pure text score);
/// the others trade relevance for freshness or the coarse popularity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortStrategy {
    #[default]
    Relevance,
    Recency,
    Popularity,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_relevance() {
        assert_eq!(SortStrategy::default(), SortStrategy::Relevance);
    }
}
