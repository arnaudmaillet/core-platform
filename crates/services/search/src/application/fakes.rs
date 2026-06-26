//! A version-aware in-memory [`SearchIndex`] plus a [`Fixture`] that wires it into
//! the handlers. Test-only (`#[cfg(test)]`) — it proves the application layer works
//! against the port contract with no engine, and in particular that the two
//! independent version guards (content vs visibility) and the cross-topic
//! out-of-order cases behave as the port documents.

// Fakes are constructed via `new()` / `Fixture::new()`; a `Default` impl would add
// noise without a caller.
#![allow(clippy::new_without_default)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use super::command::ProjectionHandler;
use super::port::{SearchIndex, WriteOutcome};
use super::query::{SearchHandler, SuggestHandler};
use crate::domain::{
    AuthorId, DocVersion, EntityKind, HitDisplay, IndexDocument, PostEvent, PostSnapshot,
    ProfileEvent, ProfileSnapshot, Searchable, SearchHit, SearchQuery, SearchResults, SourceEvent,
    SuggestQuery, Suggestion, Suggestions,
};
use crate::error::SearchError;

/// A fixed reference instant for deterministic tests.
pub fn t0() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0).unwrap()
}

// ── Test event builders ───────────────────────────────────────────────────────

/// A `PostPublished` source event with a given revision (the content version).
pub fn post_event(post_id: &str, author_id: &str, caption: &str, revision: u64) -> SourceEvent {
    SourceEvent::Post(PostEvent::Published(PostSnapshot {
        post_id: post_id.to_owned(),
        author_id: author_id.to_owned(),
        author_handle: author_id.to_owned(),
        caption: caption.to_owned(),
        hashtags: vec![],
        thumbnail_key: format!("thumbs/{post_id}.jpg"),
        created_at: t0(),
        revision,
    }))
}

/// A `ProfileUpserted` source event. An empty `profile_id` yields a malformed
/// event (for the error-path test).
pub fn profile_event(profile_id: &str, handle: &str, revision: u64) -> SourceEvent {
    SourceEvent::Profile(ProfileEvent::Upserted(ProfileSnapshot {
        profile_id: profile_id.to_owned(),
        handle: handle.to_owned(),
        display_name: handle.to_owned(),
        bio: String::new(),
        avatar_key: format!("avatars/{profile_id}.jpg"),
        verified: false,
        created_at: t0(),
        revision,
    }))
}

// ── In-memory index ───────────────────────────────────────────────────────────

/// One stored slot. `doc`/`content_version` are `None` until the content event
/// arrives — a visibility-only placeholder created by a hide that raced ahead.
struct Entry {
    author_id: Option<AuthorId>,
    content_version: Option<DocVersion>,
    visibility_version: DocVersion,
    searchable: Searchable,
    doc: Option<IndexDocument>,
}

pub struct InMemorySearchIndex {
    store: Mutex<HashMap<(EntityKind, String), Entry>>,
}

impl InMemorySearchIndex {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    // ── Assertion helpers ─────────────────────────────────────────────────────

    /// Whether a *content* document exists at this key (placeholders don't count).
    pub fn contains(&self, kind: EntityKind, id: &str) -> bool {
        self.store
            .lock()
            .unwrap()
            .get(&(kind, id.to_owned()))
            .is_some_and(|e| e.doc.is_some())
    }

    /// Whether the slot is currently visible (indexed AND searchable).
    pub fn is_visible(&self, kind: EntityKind, id: &str) -> bool {
        self.store
            .lock()
            .unwrap()
            .get(&(kind, id.to_owned()))
            .is_some_and(|e| e.doc.is_some() && e.searchable.is_visible())
    }

    /// The stored caption (post only), for asserting content state.
    pub fn caption(&self, kind: EntityKind, id: &str) -> Option<String> {
        match self.store.lock().unwrap().get(&(kind, id.to_owned()))?.doc {
            Some(IndexDocument::Post(ref d)) => Some(d.caption.clone()),
            _ => None,
        }
    }

    /// Force a slot hidden directly (used to set up the "hidden docs excluded" test
    /// without threading a moderation event).
    pub fn force_hidden(&self, kind: EntityKind, id: &str) {
        if let Some(e) = self.store.lock().unwrap().get_mut(&(kind, id.to_owned())) {
            e.searchable = Searchable::HIDDEN;
        }
    }
}

#[async_trait]
impl SearchIndex for InMemorySearchIndex {
    async fn upsert(&self, document: &IndexDocument) -> Result<WriteOutcome, SearchError> {
        let key = (document.kind(), document.id().to_owned());
        let mut store = self.store.lock().unwrap();
        match store.get(&key) {
            // Content already present and not strictly newer ⇒ external-version reject.
            Some(e)
                if e
                    .content_version
                    .is_some_and(|cv| !document.version().is_newer_than(&cv)) =>
            {
                Ok(WriteOutcome::RejectedStale)
            }
            // Existing slot (content or placeholder): replace content, PRESERVE
            // the stored moderation visibility.
            Some(e) => {
                let searchable = e.searchable;
                let visibility_version = e.visibility_version;
                store.insert(
                    key,
                    Entry {
                        author_id: document.author_id().cloned(),
                        content_version: Some(document.version()),
                        visibility_version,
                        searchable,
                        doc: Some(document.clone()),
                    },
                );
                Ok(WriteOutcome::Applied)
            }
            // First time seen: seed visibility from the document.
            None => {
                store.insert(
                    key,
                    Entry {
                        author_id: document.author_id().cloned(),
                        content_version: Some(document.version()),
                        visibility_version: DocVersion::new(0),
                        searchable: document.searchable(),
                        doc: Some(document.clone()),
                    },
                );
                Ok(WriteOutcome::Applied)
            }
        }
    }

    async fn set_searchable(
        &self,
        kind: EntityKind,
        id: &str,
        searchable: Searchable,
        version: DocVersion,
    ) -> Result<WriteOutcome, SearchError> {
        let key = (kind, id.to_owned());
        let mut store = self.store.lock().unwrap();
        match store.get_mut(&key) {
            Some(e) if !version.is_newer_than(&e.visibility_version) => {
                Ok(WriteOutcome::RejectedStale)
            }
            Some(e) => {
                e.searchable = searchable;
                e.visibility_version = version;
                Ok(WriteOutcome::Applied)
            }
            // Hide/show raced ahead of content: record a visibility-only placeholder.
            None => {
                store.insert(
                    key,
                    Entry {
                        author_id: None,
                        content_version: None,
                        visibility_version: version,
                        searchable,
                        doc: None,
                    },
                );
                Ok(WriteOutcome::Applied)
            }
        }
    }

    async fn delete(&self, kind: EntityKind, id: &str) -> Result<(), SearchError> {
        self.store.lock().unwrap().remove(&(kind, id.to_owned()));
        Ok(())
    }

    async fn purge_by_author(&self, author_id: &AuthorId) -> Result<u64, SearchError> {
        let mut store = self.store.lock().unwrap();
        let before = store.len();
        store.retain(|_, e| e.author_id.as_ref() != Some(author_id));
        Ok((before - store.len()) as u64)
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError> {
        let store = self.store.lock().unwrap();
        let mut hits: Vec<SearchHit> = Vec::new();
        for entry in store.values() {
            let Some(doc) = entry.doc.as_ref() else {
                continue; // visibility-only placeholder, no content to match
            };
            if !entry.searchable.is_visible() {
                continue;
            }
            if !query.kinds.is_empty() && !query.kinds.contains(&doc.kind()) {
                continue;
            }
            if excluded(doc, query) || !matches_text(doc, &query.text) {
                continue;
            }
            hits.push(build_hit(doc));
        }

        let estimated_total = hits.len() as u64;
        hits.truncate(query.page_size as usize);
        Ok(SearchResults {
            hits,
            next_page_token: None,
            estimated_total,
            degraded: false,
        })
    }

    async fn suggest(&self, query: &SuggestQuery) -> Result<Suggestions, SearchError> {
        let prefix = query.prefix.to_lowercase();
        let store = self.store.lock().unwrap();
        let mut suggestions: Vec<Suggestion> = Vec::new();
        for entry in store.values() {
            if suggestions.len() >= query.limit as usize {
                break;
            }
            let Some(doc) = entry.doc.as_ref() else {
                continue;
            };
            if !entry.searchable.is_visible() {
                continue;
            }
            if !query.kinds.is_empty() && !query.kinds.contains(&doc.kind()) {
                continue;
            }
            if let Some(s) = completion(doc, &prefix) {
                suggestions.push(s);
            }
        }
        Ok(Suggestions { suggestions })
    }
}

fn excluded(doc: &IndexDocument, query: &SearchQuery) -> bool {
    doc.author_id()
        .is_some_and(|a| query.exclude_author_ids.contains(a))
}

fn matches_text(doc: &IndexDocument, query: &str) -> bool {
    let needle = query.to_lowercase();
    let haystack = match doc {
        IndexDocument::Profile(d) => format!("{} {} {}", d.handle, d.display_name, d.bio),
        IndexDocument::Post(d) => format!("{} {}", d.caption, d.hashtags.join(" ")),
        IndexDocument::Hashtag(d) => d.tag.clone(),
    };
    haystack.to_lowercase().contains(&needle)
}

fn build_hit(doc: &IndexDocument) -> SearchHit {
    match doc {
        IndexDocument::Profile(d) => SearchHit {
            kind: EntityKind::Profile,
            id: d.profile_id.clone(),
            score: 1.0,
            snippet: d.bio.clone(),
            display: HitDisplay::Profile {
                handle: d.handle.clone(),
                display_name: d.display_name.clone(),
                avatar_key: d.avatar_key.clone(),
                verified: d.verified,
            },
        },
        IndexDocument::Post(d) => SearchHit {
            kind: EntityKind::Post,
            id: d.post_id.clone(),
            score: 1.0,
            snippet: d.caption.clone(),
            display: HitDisplay::Post {
                author_id: d.author_id.as_str().to_owned(),
                author_handle: d.author_handle.clone(),
                thumbnail_key: d.thumbnail_key.clone(),
                created_at: d.created_at,
            },
        },
        IndexDocument::Hashtag(d) => SearchHit {
            kind: EntityKind::Hashtag,
            id: d.tag.clone(),
            score: 1.0,
            snippet: String::new(),
            display: HitDisplay::Hashtag {
                tag: d.tag.clone(),
                post_count: d.post_count,
            },
        },
    }
}

fn completion(doc: &IndexDocument, prefix: &str) -> Option<Suggestion> {
    let (text, id) = match doc {
        IndexDocument::Profile(d) => (d.handle.clone(), Some(d.profile_id.clone())),
        IndexDocument::Hashtag(d) => (d.tag.clone(), None),
        // Posts have no single completion token.
        IndexDocument::Post(_) => return None,
    };
    text.to_lowercase().starts_with(prefix).then(|| Suggestion {
        kind: doc.kind(),
        text,
        id,
        score: 1.0,
    })
}

// ── Fixture ───────────────────────────────────────────────────────────────────

pub struct Fixture {
    pub index: Arc<InMemorySearchIndex>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            index: Arc::new(InMemorySearchIndex::new()),
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        t0()
    }

    pub fn projection_handler(&self) -> ProjectionHandler {
        ProjectionHandler::new(self.index.clone())
    }

    pub fn search_handler(&self) -> SearchHandler {
        SearchHandler::new(self.index.clone())
    }

    pub fn suggest_handler(&self) -> SuggestHandler {
        SuggestHandler::new(self.index.clone())
    }
}
