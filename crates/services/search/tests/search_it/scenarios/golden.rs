//! Golden-query relevance suite — the guard against ranking regressions.
//!
//! A frozen corpus + a set of expected orderings: if an analyzer change, a field
//! boost, or a query-DSL tweak silently changes relevance, one of these fails.
//! Runs only against OpenSearch (the canonical engine; relevance is never asserted
//! against the Meilisearch dev adapter).
//!
//! The assertions here are deliberately *structural* (exact-match dominance, boost
//! ordering, topical filtering) so they hold across BM25 retuning. Tightening them
//! to exact top-k id lists is a calibration step to do against a live cluster — the
//! same way the fuzzy-match threshold is calibrated.

use crate::search_it::harness::{Harness, ids};

/// Seeds the shared golden corpus.
async fn seed(h: &Harness) {
    // Profiles.
    h.index_profile("p-alice", "alicedev", "Alice Dev", "rust systems engineer", 1)
        .await;
    h.index_profile("p-bob", "bobchef", "Bob Chef", "italian home cooking", 1)
        .await;
    // Posts.
    h.index_post("post-rust", "acct-1", "rust async runtime internals", 1)
        .await;
    h.index_post("post-pasta", "acct-2", "weeknight pasta recipes", 1)
        .await;
    // Hashtags (tag is boosted ^3, like handle).
    h.index_hashtag("rust", 1000, 1).await;
    h.refresh().await;
}

#[tokio::test]
async fn exact_handle_match_is_the_top_hit() {
    let h = Harness::start().await;
    seed(&h).await;
    let resp = h.search("alicedev").await;
    assert_eq!(
        resp.hits.first().map(|x| x.id.as_str()),
        Some("p-alice"),
        "an exact handle match must rank first"
    );
}

#[tokio::test]
async fn boosted_tag_outranks_an_incidental_caption_mention() {
    let h = Harness::start().await;
    seed(&h).await;
    let resp = h.search("rust").await;
    // The `rust` hashtag (tag^3) should outrank a post that merely mentions rust.
    assert_eq!(
        resp.hits.first().map(|x| x.id.as_str()),
        Some("rust"),
        "the boosted exact tag must top a caption mention"
    );
}

#[tokio::test]
async fn irrelevant_documents_are_filtered_out() {
    let h = Harness::start().await;
    seed(&h).await;
    let result = ids(&h.search("rust").await);
    assert!(
        result.contains(&"post-rust".to_owned()),
        "topical post should match"
    );
    assert!(
        !result.contains(&"post-pasta".to_owned()),
        "an off-topic post must not appear for 'rust'"
    );
}
