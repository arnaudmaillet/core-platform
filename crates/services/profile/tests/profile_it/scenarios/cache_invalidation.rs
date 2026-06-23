//! Scenario — Redis profile-cache invalidation.
//!
//! `GetProfileById` is read-through: a miss reads ScyllaDB and warms the Redis
//! cache. A mutation (`UpdateProfile`) must invalidate that cache entry so the
//! next read reflects the new value rather than a stale one. This is the
//! cache-invalidation axis: warm → mutate → bust → re-read fresh.

use crate::profile_it::harness::{self, TestHarness, DEADLINE};
use crate::profile_it::harness::ProfileId;

#[tokio::test]
async fn update_busts_the_read_through_cache() {
    let h = TestHarness::start().await;

    let account = harness::random_account_id();
    let handle = harness::random_handle();
    h.create(&account, &handle, "Alice").await;

    let view = h.get_by_handle(&handle).await.expect("profile exists after create");
    let id = view.id.clone();
    let pid = ProfileId::try_from(id.as_str()).expect("valid profile id");

    // Read-through warms the Redis cache.
    let read = h.get_by_id(&id).await.expect("get_by_id");
    assert_eq!(read.display_name, "Alice");

    let cache = h.cache.clone();
    harness::await_until("read-through warmed the profile cache", DEADLINE, || {
        let cache = cache.clone();
        async move { cache.get_by_id(&pid).await.map(|v| v.is_some()).unwrap_or(false) }
    })
    .await;

    // Mutating the display name must invalidate the cached entry.
    h.update_display(&id, "Bob").await;

    harness::await_until("update invalidated the cached entry", DEADLINE, || {
        let cache = cache.clone();
        async move { cache.get_by_id(&pid).await.map(|v| v.is_none()).unwrap_or(false) }
    })
    .await;

    // The next read returns the fresh value (and re-warms the cache).
    let reread = h.get_by_id(&id).await.expect("get_by_id");
    assert_eq!(reread.display_name, "Bob", "read after update must reflect the new display name");
}
