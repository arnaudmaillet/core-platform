//! Refresh rotation + reuse-detection against real Postgres + Redis.

use tonic::Code;

use crate::auth_it::harness::{random_user, Harness};

#[tokio::test]
async fn refresh_rotates_to_a_new_pair() {
    let h = Harness::start().await;
    let login = h.login(&random_user()).await.expect("login");
    let first = login.tokens.unwrap();

    let refreshed = h.refresh(&first.refresh_token).await.expect("refresh").tokens.unwrap();
    assert_eq!(refreshed.session_id, first.session_id, "same session");
    assert_ne!(refreshed.refresh_token, first.refresh_token, "refresh token rotates");
    assert!(!refreshed.access_token.is_empty());

    // The rotated successor still works.
    let again = h.refresh(&refreshed.refresh_token).await.expect("second refresh");
    assert!(again.tokens.is_some());
}

#[tokio::test]
async fn reusing_a_rotated_refresh_token_revokes_the_whole_generation() {
    let h = Harness::start().await;
    let login = h.login(&random_user()).await.expect("login");
    let account_id = login.account_id.clone();
    let original = login.tokens.unwrap();

    // Legitimate rotation: `original` is now spent.
    let _rotated = h.refresh(&original.refresh_token).await.expect("rotate").tokens.unwrap();

    // Re-presenting the spent token is treated as theft.
    let status = h.refresh(&original.refresh_token).await.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);

    // The whole session is gone: no active sessions remain for the account.
    assert_eq!(h.count_active_sessions(&account_id).await, 0);
    assert!(h.list_sessions(&account_id).await.expect("list").sessions.is_empty());
}

#[tokio::test]
async fn unknown_refresh_token_is_unauthenticated() {
    let h = Harness::start().await;
    let status = h.refresh("nonexistent-token").await.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}
