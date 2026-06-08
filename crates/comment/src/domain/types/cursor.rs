// crates/content_comments/src/domain/types/comment_cursor.rs
use shared_kernel::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use uuid::Uuid;
use chrono::{DateTime, Utc, TimeZone};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentCursor {
    pub created_at: DateTime<Utc>,
    pub comment_id: Uuid,
}

impl CommentCursor {
    pub fn new(created_at: DateTime<Utc>, comment_id: Uuid) -> Self {
        Self { created_at, comment_id }
    }

    /// Encode le curseur en une chaîne Base64 opaque pour le client API
    pub fn to_token(&self) -> String {
        let millis = self.created_at.timestamp_millis();
        let raw_string = format!("{}:{}", millis, self.comment_id);
        URL_SAFE_NO_PAD.encode(raw_string)
    }

    /// Décode le token reçu de l'API pour récupérer les valeurs pivots pour ScyllaDB
    pub fn try_from_token(token: &str) -> Result<Self> {
        let decoded_bytes = URL_SAFE_NO_PAD.decode(token)
            .map_err(|_| Error::validation("cursor", "Invalid base64 encoding"))?;
        
        let decoded_str = String::from_utf8(decoded_bytes)
            .map_err(|_| Error::validation("cursor", "Invalid utf8 sequence"))?;

        let parts: Vec<&str> = decoded_str.split(':').collect();
        if parts.len() != 2 {
            return Err(Error::validation("cursor", "Invalid cursor format"));
        }

        let millis = parts[0].parse::<i64>()
            .map_err(|_| Error::validation("cursor", "Invalid timestamp in cursor"))?;
        
        let comment_id = Uuid::parse_str(parts[1])
            .map_err(|_| Error::validation("cursor", "Invalid UUID in cursor"))?;

        let created_at = Utc.timestamp_millis_opt(millis)
            .single()
            .ok_or_else(|| Error::validation("cursor", "Timestamp out of range"))?;

        Ok(Self { created_at, comment_id })
    }
}

impl ValueObject for CommentCursor {
    fn validate(&self) -> Result<()> {
        if self.comment_id.is_nil() {
            return Err(Error::validation("cursor", "Cursor Comment ID cannot be nil"));
        }
        Ok(())
    }
}