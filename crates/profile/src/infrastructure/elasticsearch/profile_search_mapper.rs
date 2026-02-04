// crates/profile/src/infrastructure/elasticsearch/profile_search_mapper.rs

use super::profile_search_document::{AutocompleteSuggest, ProfileSearchDocument};

pub struct ProfileSearchMapper;

impl ProfileSearchMapper {
    pub fn to_search_document(
        account_id: &str,
        username: &str,
        display_name: &str,
        avatar_url: Option<&str>,
        occurred_at: &chrono::DateTime<chrono::Utc>,
    ) -> ProfileSearchDocument {
        ProfileSearchDocument {
            account_id: account_id.to_string(),
            username: username.to_string(),
            display_name: display_name.to_string(),
            avatar_url: avatar_url.map(|s| s.to_string()),
            suggest: AutocompleteSuggest {
                input: vec![username.to_string(), display_name.to_string()],
                weight: 10,
            },
            updated_at: occurred_at.to_rfc3339(),
        }
    }
}
