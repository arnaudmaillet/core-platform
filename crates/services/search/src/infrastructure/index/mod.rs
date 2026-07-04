//! The OpenSearch engine adapter and its versioned mapping artifacts.

pub mod mappings;
pub mod opensearch;

pub use opensearch::{OpenSearchConfig, OpenSearchIndex};
