//! Moderation visibility against the live engine: a hide removes a document from
//! results (retaining it), a reversal restores it, and — the two-version-guard
//! invariant — a hide survives a later content edit.

use search::domain::EntityKind;

use crate::search_it::harness::Harness;

#[tokio::test]
async fn hide_removes_from_results_and_restore_brings_it_back() {
    let h = Harness::start().await;
    h.index_post("p1", "acct-1", "rustacean musings", 1).await;
    assert_eq!(h.search_ids("rustacean").await, vec!["p1".to_owned()]);

    h.hide(EntityKind::Post, "p1", 200).await;
    h.refresh().await;
    assert!(
        h.search("rustacean").await.hits.is_empty(),
        "a hidden document must not appear in results"
    );

    h.restore(EntityKind::Post, "p1", 300).await;
    assert_eq!(
        h.search_ids("rustacean").await,
        vec!["p1".to_owned()],
        "a reversal restores the retained document"
    );
}

#[tokio::test]
async fn moderation_hide_survives_a_later_content_edit() {
    let h = Harness::start().await;
    h.index_post("p1", "acct-1", "original text", 1).await;
    h.hide(EntityKind::Post, "p1", 200).await;
    h.refresh().await;
    assert!(h.search("original").await.hits.is_empty());

    // A newer content edit lands while hidden: content updates, visibility persists.
    h.index_post("p1", "acct-1", "edited text", 2).await;
    h.refresh().await;
    assert!(
        h.search("edited").await.hits.is_empty(),
        "a content re-projection must not un-hide a moderated document"
    );

    // Once reversed, the *new* content is what surfaces.
    h.restore(EntityKind::Post, "p1", 400).await;
    assert_eq!(h.search_ids("edited").await, vec!["p1".to_owned()]);
    assert!(
        h.search("original").await.hits.is_empty(),
        "the edit took effect under the hood"
    );
}
