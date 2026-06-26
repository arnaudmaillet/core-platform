//! External versioning: the engine's `version_type=external` guard arbitrates
//! out-of-order writes — a stale edit can never clobber a newer document, and a
//! newer edit fully re-projects it.

use crate::search_it::harness::Harness;

#[tokio::test]
async fn stale_edit_is_rejected_and_newer_edit_reprojects() {
    let h = Harness::start().await;

    // v5 content.
    h.index_post("p1", "acct-1", "alpha content", 5).await;
    assert_eq!(h.search_ids("alpha").await, vec!["p1".to_owned()]);

    // A stale v3 edit arrives late — the external-version guard rejects it.
    h.index_post("p1", "acct-1", "beta content", 3).await;
    h.refresh().await;
    assert!(
        h.search("beta").await.hits.is_empty(),
        "a stale (lower-version) edit must not overwrite newer content"
    );
    assert_eq!(
        h.search_ids("alpha").await,
        vec!["p1".to_owned()],
        "content stays at the newer version"
    );

    // A newer v7 edit fully re-projects the document.
    h.index_post("p1", "acct-1", "gamma content", 7).await;
    assert_eq!(h.search_ids("gamma").await, vec!["p1".to_owned()]);
    assert!(
        h.search("alpha").await.hits.is_empty(),
        "the old content is gone after a real edit"
    );
}
