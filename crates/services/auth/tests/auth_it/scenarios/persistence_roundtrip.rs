//! The durable write path: a login persists a session, a refresh token, and the
//! immutable subject link; a refresh updates the rotation lineage in place.

use crate::auth_it::harness::{random_user, Harness};

#[tokio::test]
async fn login_persists_session_refresh_and_link() {
    let h = Harness::start().await;
    let login = h.login(&random_user()).await.expect("login");
    let account_id = login.account_id.clone();
    let tokens = login.tokens.unwrap();

    // Session row.
    let session_id = uuid::Uuid::parse_str(&tokens.session_id).unwrap();
    let session_account: uuid::Uuid =
        sqlx::query_scalar("SELECT account_id FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&h.pool)
            .await
            .expect("session row exists");
    assert_eq!(session_account.to_string(), account_id);

    // Exactly one active refresh token for the session.
    let refresh_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE session_id = $1 AND status = 'active'")
            .bind(session_id)
            .fetch_one(&h.pool)
            .await
            .expect("count refresh tokens");
    assert_eq!(refresh_count, 1);

    // Immutable subject link.
    assert_eq!(h.count_subject_links(&account_id).await, 1);
}

#[tokio::test]
async fn refresh_marks_the_old_token_rotated_and_chains_lineage() {
    let h = Harness::start().await;
    let login = h.login(&random_user()).await.expect("login");
    let tokens = login.tokens.unwrap();
    let session_id = uuid::Uuid::parse_str(&tokens.session_id).unwrap();

    h.refresh(&tokens.refresh_token).await.expect("refresh");

    // After one rotation: one rotated (with a successor) + one active.
    let rotated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE session_id = $1 AND status = 'rotated' AND replaced_by IS NOT NULL",
    )
    .bind(session_id)
    .fetch_one(&h.pool)
    .await
    .expect("count rotated");
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE session_id = $1 AND status = 'active'")
            .bind(session_id)
            .fetch_one(&h.pool)
            .await
            .expect("count active");
    assert_eq!(rotated, 1, "original is rotated with a successor");
    assert_eq!(active, 1, "exactly one live token");
}
