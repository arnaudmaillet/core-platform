//! Environment-sourced configuration, resolved once at boot and threaded into the
//! composition root ([`crate::app::App::build`]). Kafka connection config is
//! resolved separately via `KafkaClientConfig::from_env`.

use crate::infrastructure::index::OpenSearchConfig;

/// Fully-resolved search configuration.
pub struct SearchConfig {
    pub opensearch: OpenSearchConfig,
    /// gRPC endpoint of the `post` service, used by the ingestion hydrator to fetch
    /// the authoritative snapshot behind a thin `post.v1.events` notification.
    pub post_endpoint: String,
}

impl SearchConfig {
    pub fn from_env() -> Self {
        let mut opensearch = OpenSearchConfig::new(
            env_or("SEARCH_OPENSEARCH_URL", "http://localhost:9200"),
            env_or("SEARCH_INDEX_PREFIX", "search"),
        );
        if let Ok(raw) = std::env::var("SEARCH_QUERY_TIMEOUT_MS")
            && let Ok(ms) = raw.parse::<u64>()
        {
            opensearch = opensearch.with_request_timeout(std::time::Duration::from_millis(ms));
        }
        if let (Ok(user), Ok(pass)) = (
            std::env::var("SEARCH_OPENSEARCH_USER"),
            std::env::var("SEARCH_OPENSEARCH_PASSWORD"),
        ) {
            opensearch = opensearch.with_basic_auth(user, pass);
        }
        Self {
            opensearch,
            post_endpoint: env_or("SEARCH_POST_GRPC_ENDPOINT", "http://localhost:50056"),
        }
    }
}

fn env_or(key: &str, default: impl Into<String>) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}
