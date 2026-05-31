// crates/auth/tests/integration.rs

use auth::TokenValidator;
use auth::domain::validator::AuthError;
use auth_test_utils::KeycloakTestContext;
use shared_kernel::{core::Result, security::JwtToken};

#[tokio::test]
async fn test_integration_keycloak_discovery() {
    // Utilise le Singleton : boot 20s la première fois, 0s les suivantes
    let ctx: KeycloakTestContext =
        KeycloakTestContext::restore("master", "account-service".to_string()).await;

    // Si KeycloakValidator::new a réussi, c'est que le Discovery (HTTP + Parsing) est OK
    assert!(ctx.uri.starts_with("http://"));

    let result = TokenValidator::validate(
        ctx.validator.as_ref(),
        &JwtToken::from_raw("invalid.token.structure"),
    );
    assert!(matches!(result, Err(AuthError::InvalidToken)));
}

#[tokio::test]
async fn test_keycloak_discovery_works_with_singleton() {
    // 1. On restaure (ou crée) le contexte.
    // Si c'est le premier test, il lance Docker.
    // Sinon, il réutilise l'instance existante instantanément.
    let ctx = KeycloakTestContext::restore("master", "account-service".to_string()).await;

    // 2. On utilise le validateur déjà prêt dans le contexte
    let fake_token = JwtToken::from_raw("header.payload.signature");
    let result = ctx.validator.validate(&fake_token);

    // 3. Assertions
    // Ici on vérifie que le validateur a bien réussi son Discovery (JWKS)
    // et qu'il est capable d'analyser un token (même s'il est invalide ici)
    assert!(result.is_err());
    println!(
        "✅ Discovery successful and validator is active on {}",
        ctx.uri
    );
}

#[tokio::test]
async fn test_another_realm_reuse_container() -> Result<()> {
    // Ce test va s'exécuter immédiatement sans attendre 20s de boot Docker
    let ctx = KeycloakTestContext::restore("master", "account-service".to_string()).await;

    assert_eq!(ctx.realm, "master");
    assert!(ctx.uri.starts_with("http://127.0.0.1:"));

    Ok(())
}
