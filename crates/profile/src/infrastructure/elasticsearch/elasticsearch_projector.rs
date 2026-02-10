use crate::domain::events::ProfileEvent;
use crate::infrastructure::elasticsearch::{AutocompleteSuggest, ProfileSearchDocument};
use chrono::{DateTime, Utc};
use elasticsearch::{
    Elasticsearch, IndexParts, UpdateParts,
    indices::{IndicesCreateParts, IndicesExistsParts},
    params::OpType,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct ProfileElasticProjector {
    client: Elasticsearch,
    index_name: &'static str,
}

impl ProfileElasticProjector {
    pub fn new(client: Elasticsearch) -> Self {
        Self {
            client,
            index_name: "profiles_v1",
        }
    }

    /// Initialise l'index avec le mapping "Type-Ahead" (Edge N-Grams)
    pub async fn ensure_index_ready(&self) -> anyhow::Result<()> {
        let exists = self
            .client
            .indices()
            .exists(IndicesExistsParts::Index(&[self.index_name]))
            .send()
            .await?
            .status_code()
            .is_success();

        if !exists {
            self.create_index().await?;
            tracing::info!(
                "Elasticsearch index '{}' created with Edge N-Gram mapping",
                self.index_name
            );
        }
        Ok(())
    }

    /// Traite les événements du domaine (reçoit des Value Objects)
    pub async fn project(&self, event: &ProfileEvent) -> anyhow::Result<()> {
        match event {
            ProfileEvent::ProfileCreated {
                profile_id,
                display_name,
                handle,
                occurred_at,
                ..
            } => {
                let doc = self.map_to_doc(
                    &profile_id.to_string(),
                    handle.as_str(),
                    display_name.as_str(),
                    None,
                    occurred_at,
                );
                self.upsert_document(doc).await
            }

            ProfileEvent::HandleChanged {
                profile_id,
                new_handle,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "handle": new_handle.as_str(),
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::DisplayNameChanged {
                profile_id,
                new_display_name,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "display_name": new_display_name.as_str(),
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::BioUpdated {
                profile_id,
                new_bio,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "bio": new_bio.as_ref().map(|b| b.as_str()),
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::AvatarUpdated {
                profile_id,
                new_avatar_url,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "avatar_url": new_avatar_url.as_str(),
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::AvatarRemoved {
                profile_id,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "avatar_url": serde_json::Value::Null,
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::BannerUpdated {
                profile_id,
                new_banner_url,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "banner_url": new_banner_url.as_str(),
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            ProfileEvent::BannerRemoved {
                profile_id,
                occurred_at,
                ..
            } => {
                self.update_partial(
                    &profile_id.to_string(),
                    json!({
                        "banner_url": serde_json::Value::Null,
                        "updated_at": occurred_at.to_rfc3339()
                    }),
                )
                .await
            }

            _ => Ok(()),
        }
    }

    // --- Private Helpers ---

    async fn create_index(&self) -> anyhow::Result<()> {
        self.client
            .indices()
            .create(IndicesCreateParts::Index(self.index_name))
            .body(json!({
                "settings": {
                    "index": {
                        "number_of_shards": 3,
                        "number_of_replicas": 1
                    },
                    "analysis": {
                        "analyzer": {
                            "autocomplete_analyzer": {
                                "type": "custom",
                                "tokenizer": "edge_ngram_tokenizer",
                                "filter": ["lowercase"]
                            }
                        },
                        "tokenizer": {
                            "edge_ngram_tokenizer": {
                                "type": "edge_ngram",
                                "min_gram": 2,
                                "max_gram": 20,
                                "token_chars": ["letter", "digit"]
                            }
                        }
                    }
                },
                "mappings": {
                    "properties": {
                        "profile_id": { "type": "keyword" },
                        "handle": {
                            "type": "text",
                            "analyzer": "autocomplete_analyzer",
                            "search_analyzer": "standard"
                        },
                        "display_name": { "type": "text" },
                        "avatar_url": { "type": "keyword", "index": false },
                        "suggest": { "type": "completion" },
                        "updated_at": { "type": "date" }
                    }
                }
            }))
            .send()
            .await?;
        Ok(())
    }

    fn map_to_doc(
        &self,
        profile_id: &str,
        handle: &str,
        name: &str,
        avatar: Option<&str>,
        ts: &DateTime<Utc>,
    ) -> ProfileSearchDocument {
        ProfileSearchDocument {
            profile_id: profile_id.to_string(),
            handle: handle.to_string(),
            display_name: name.to_string(),
            avatar_url: avatar.map(|s| s.to_string()),
            suggest: AutocompleteSuggest {
                input: vec![handle.to_string(), name.to_string()],
                weight: 10,
            },
            updated_at: ts.to_rfc3339(),
        }
    }

    async fn upsert_document(&self, doc: ProfileSearchDocument) -> anyhow::Result<()> {
        self.client
            .index(IndexParts::IndexId(self.index_name, &doc.profile_id))
            .op_type(OpType::Index) // Ecrase si existe (Idempotence)
            .body(serde_json::to_value(&doc)?)
            .send()
            .await?;
        Ok(())
    }

    async fn update_partial(&self, id: &str, partial_body: Value) -> anyhow::Result<()> {
        self.client
            .update(UpdateParts::IndexId(self.index_name, id))
            .body(json!({ "doc": partial_body }))
            .send()
            .await?;
        Ok(())
    }
}
