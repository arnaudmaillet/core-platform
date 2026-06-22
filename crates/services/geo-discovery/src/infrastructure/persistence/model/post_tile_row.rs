use scylla::DeserializeRow;
use uuid::Uuid;

/// ScyllaDB row type for `geo_discovery.posts_by_tile`.
///
/// Only `post_id` is selected in cold-start recovery queries (the tile key
/// columns are already known from the query parameters).
#[derive(Debug, DeserializeRow)]
pub struct PostTileRow {
    pub post_id: Uuid,
}
