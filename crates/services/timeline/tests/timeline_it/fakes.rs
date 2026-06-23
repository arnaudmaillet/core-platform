//! In-process fake for the social-graph gRPC dependency.
//!
//! Timeline's [`SocialGraphClient`] port is the seam between the read/fan-out
//! paths and the social-graph service. Rather than boot a second microservice,
//! the suite injects [`FakeSocialGraph`]: it *is* the follow graph (tests declare
//! edges with [`add_follow`](FakeSocialGraph::add_follow)) and it counts calls, so
//! a scenario can assert that, e.g., a warmed following-set is not rebuilt from
//! gRPC again.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;

use timeline::application::port::SocialGraphClient;
use timeline::domain::value_object::{AuthorId, ProfileId};
use timeline::error::TimelineError;

/// A deterministic, call-counting stand-in for the social-graph service.
#[derive(Default)]
pub struct FakeSocialGraph {
    /// author → its followers (drives fan-out-on-write).
    followers: Mutex<HashMap<AuthorId, Vec<ProfileId>>>,
    /// profile → who it follows (drives cold-start following-set rebuild).
    following: Mutex<HashMap<ProfileId, Vec<AuthorId>>>,
    followers_calls: AtomicUsize,
    following_calls: AtomicUsize,
}

impl FakeSocialGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares that `follower` follows `author`, updating both projections.
    pub fn add_follow(&self, follower: ProfileId, author: AuthorId) {
        self.followers.lock().unwrap().entry(author).or_default().push(follower);
        self.following.lock().unwrap().entry(follower).or_default().push(author);
    }

    /// Number of `list_all_followers` calls observed so far.
    pub fn followers_calls(&self) -> usize {
        self.followers_calls.load(Ordering::SeqCst)
    }

    /// Number of `list_all_following` calls observed so far.
    pub fn following_calls(&self) -> usize {
        self.following_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SocialGraphClient for FakeSocialGraph {
    async fn list_all_followers(
        &self,
        author_id: &AuthorId,
        _page_size: i32,
    ) -> Result<Vec<ProfileId>, TimelineError> {
        self.followers_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.followers.lock().unwrap().get(author_id).cloned().unwrap_or_default())
    }

    async fn list_all_following(
        &self,
        profile_id: &ProfileId,
        _page_size: i32,
    ) -> Result<Vec<AuthorId>, TimelineError> {
        self.following_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.following.lock().unwrap().get(profile_id).cloned().unwrap_or_default())
    }
}
