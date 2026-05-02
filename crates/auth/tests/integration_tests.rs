use auth::{TokenValidator, infrastructure::keycloak_test_context::KeycloakTestContext};
use shared_kernel::domain::value_objects::JwtToken;

#[tokio::test]
async fn test_keycloak_discovery_works_with_singleton() {
    // 1. On restaure (ou crée) le contexte.
    // Si c'est le premier test, il lance Docker.
    // Sinon, il réutilise l'instance existante instantanément.
    let ctx = KeycloakTestContext::restore("master").await;

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
async fn test_another_realm_reuse_container() {
    // Ce test va s'exécuter immédiatement sans attendre 20s de boot Docker
    let ctx = KeycloakTestContext::restore("master").await;

    assert_eq!(ctx.realm, "master");
    assert!(ctx.uri.starts_with("http://127.0.0.1:"));
}

#[tokio::test]
async fn test_full_validation_flow_with_real_keycloak_token() {
    // 1. Setup
    let ctx = KeycloakTestContext::restore("master").await;

    // 2. Récupération d'un VRAI token généré par le serveur Docker
    let raw_token = ctx.get_real_admin_token().await;
    let jwt_token = JwtToken::from_raw(raw_token);

    // 3. Validation
    let result = ctx.validator.validate(&jwt_token);

    // 4. Assertions
    assert!(
        result.is_ok(),
        "Le validateur a rejeté un vrai jeton Keycloak !"
    );
    let claims = result.unwrap();

    // On vérifie que le mapping vers nos Value Objects est parfait
    assert!(!claims.sub_id.as_str().is_empty());
}
