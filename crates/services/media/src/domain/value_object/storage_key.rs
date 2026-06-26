use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::value_object::{AssetId, ContentHash, MediaKind, RenditionKind};

/// An object-storage key — the path of an object in the canonical byte store. It
/// is never a URL and never bytes; the delivery plane turns a key into a CDN /
/// signed URL.
///
/// Two shapes, by lifecycle:
/// * **staging** (`uploads/{asset_id}`) — where the client PUTs the raw bytes
///   before the content hash is known. Keyed by the freshly-minted asset id.
/// * **rendition** (`{kind}/{content_hash}/{slug}.{ext}`) — the **content-
///   addressed** key for a derivative. Because the hash segment is derived from
///   the bytes, the same bytes always map to the same key (cacheable forever) and
///   an edit (different bytes → different hash) lands on a brand-new key, which is
///   why public delivery URLs are immutable and never need invalidation on edit.
///
/// The scheme is a **stateful contract**: once objects exist, its meaning must
/// never change (it is load-bearing for cache correctness and dedup) — version it,
/// don't mutate it.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageKey(String);

impl StorageKey {
    /// The staging key the pre-signed upload PUTs to (pre-hash).
    pub fn staging(asset_id: AssetId) -> Self {
        Self(format!("uploads/{asset_id}"))
    }

    /// The content-addressed key for a finished rendition.
    pub fn rendition(
        kind: MediaKind,
        hash: &ContentHash,
        rendition: RenditionKind,
        extension: &str,
    ) -> Self {
        Self(format!(
            "{}/{}/{}.{}",
            kind.path_segment(),
            hash.as_str(),
            rendition.slug(),
            extension
        ))
    }

    /// Wraps a key read back from storage.
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StorageKey({})", self.0)
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn hash() -> ContentHash {
        ContentHash::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap()
    }

    #[test]
    fn staging_key_is_keyed_by_asset_id() {
        let id = AssetId::from_uuid(Uuid::from_u128(1));
        assert_eq!(
            StorageKey::staging(id).as_str(),
            format!("uploads/{id}")
        );
    }

    #[test]
    fn rendition_key_is_content_addressed() {
        let k = StorageKey::rendition(MediaKind::PostImage, &hash(), RenditionKind::Thumbnail, "webp");
        assert_eq!(
            k.as_str(),
            "post-images/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855/thumbnail.webp"
        );
    }

    #[test]
    fn identical_bytes_yield_identical_keys() {
        let a = StorageKey::rendition(MediaKind::Avatar, &hash(), RenditionKind::Original, "jpg");
        let b = StorageKey::rendition(MediaKind::Avatar, &hash(), RenditionKind::Original, "jpg");
        assert_eq!(a, b, "content-addressing must be deterministic");
    }
}
