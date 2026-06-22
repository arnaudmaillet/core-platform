use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::error::TimelineError;

/// Opaque pagination cursor for GetFollowingFeed.
///
/// Encodes the position of the last item returned in a page. The server
/// uses this to resume scanning from the correct position in subsequent
/// requests. Clients treat it as an opaque string — the encoding may change
/// between service versions.
///
/// Encoding: `base64url("{published_at_ms}:{post_id_hyphenated}")`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedCursor {
    /// Unix epoch milliseconds of the last returned feed item.
    pub published_at_ms: i64,
    /// UUID string of the last returned post (tie-breaking within the same ms).
    pub post_id_str: [u8; 36],
}

impl FeedCursor {
    pub fn new(published_at_ms: i64, post_id: &str) -> Self {
        let mut post_id_str = [0u8; 36];
        let bytes = post_id.as_bytes();
        let len = bytes.len().min(36);
        post_id_str[..len].copy_from_slice(&bytes[..len]);
        Self { published_at_ms, post_id_str }
    }

    pub fn post_id_str(&self) -> &str {
        let end = self.post_id_str.iter().position(|&b| b == 0).unwrap_or(36);
        std::str::from_utf8(&self.post_id_str[..end]).unwrap_or_default()
    }

    pub fn encode(&self) -> String {
        let raw = format!("{}:{}", self.published_at_ms, self.post_id_str());
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }

    pub fn decode(token: &str) -> Result<Self, TimelineError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| TimelineError::InvalidPageToken { token: token.to_owned() })?;

        let s = std::str::from_utf8(&bytes)
            .map_err(|_| TimelineError::InvalidPageToken { token: token.to_owned() })?;

        let (ts_str, post_id) = s.split_once(':').ok_or_else(|| {
            TimelineError::InvalidPageToken { token: token.to_owned() }
        })?;

        let published_at_ms = ts_str.parse::<i64>().map_err(|_| {
            TimelineError::InvalidPageToken { token: token.to_owned() }
        })?;

        Ok(Self::new(published_at_ms, post_id))
    }
}
