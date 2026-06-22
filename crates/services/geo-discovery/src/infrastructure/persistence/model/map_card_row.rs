use scylla::DeserializeRow;
use scylla::value::CqlTimestamp;
use uuid::Uuid;

/// ScyllaDB row type for `geo_discovery.map_post_cards`.
///
/// Column order must match the SELECT projection exactly:
///   post_id, author_id, author_handle, author_avatar_url, thumbnail_url,
///   h3_index_r7, virality_score, published_at, expires_at
#[derive(Debug, DeserializeRow)]
pub struct MapCardRow {
    pub post_id:           Uuid,
    pub author_id:         Uuid,
    pub author_handle:     String,
    pub author_avatar_url: String,
    pub thumbnail_url:     String,
    pub h3_index_r7:       i64,
    pub virality_score:    f32,
    pub published_at:      CqlTimestamp,
    pub expires_at:        CqlTimestamp,
}

impl From<MapCardRow> for crate::domain::entity::MapPostCard {
    fn from(row: MapCardRow) -> Self {
        Self {
            post_id:           row.post_id,
            author_id:         row.author_id,
            author_handle:     row.author_handle,
            author_avatar_url: row.author_avatar_url,
            thumbnail_url:     row.thumbnail_url,
            h3_index_r7:       row.h3_index_r7,
            virality_score:    row.virality_score,
            published_at_ms:   row.published_at.0,
        }
    }
}
