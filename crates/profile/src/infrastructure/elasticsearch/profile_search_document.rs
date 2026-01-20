// crates/profile/src/infrastructure/elasticsearch/profile_search_document.rs

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileSearchDocument {
    pub account_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub suggest: AutocompleteSuggest,
    pub updated_at: String, // ISO 8601
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AutocompleteSuggest {
    pub input: Vec<String>,
    pub weight: i32,
}