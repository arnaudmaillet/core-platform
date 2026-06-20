mod common;

use std::time::Duration;

use auth_context::{AuthContextConfig, AuthError, JwksCache, JwtDecoder, OidcClaimsExtractor};

use common::{TestKeyPair, TokenFactory};

const ISS: &str = "https://idp.example.com/realms/platform";
const AUD: &str = "platform-api";
const SUB: &str = "user-abc-123";

async fn seeded_cache(key: &TestKeyPair) -> JwksCache {
    let cache = JwksCache::new();
    cache.replace(key.as_cache_map()).await;
    cache
}

fn default_config() -> AuthContextConfig {
    AuthContextConfig {
        jwks_url: String::new(),
        expected_issuer: Some(ISS.to_owned()),
        expected_audience: Some(AUD.to_owned()),
        clock_skew: Duration::from_secs(5),
        ..Default::default()
    }
}

#[tokio::test]
async fn valid_token_decodes_successfully() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&default_config(), cache, OidcClaimsExtractor::default());
    let factory = TokenFactory::new(&key);

    let token = factory.valid(SUB, ISS, AUD, "openid profile");
    let principal = decoder.decode(&token).await.unwrap();

    assert_eq!(principal.user_id.as_str(), SUB);
    assert!(principal.has_permission("openid"));
    assert!(principal.has_permission("profile"));
}

#[tokio::test]
async fn expired_token_returns_token_expired_error() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;

    let config = AuthContextConfig {
        clock_skew: Duration::ZERO,
        expected_issuer: Some(ISS.to_owned()),
        expected_audience: Some(AUD.to_owned()),
        ..Default::default()
    };

    let decoder = JwtDecoder::new(&config, cache, OidcClaimsExtractor::default());
    let token = TokenFactory::new(&key).expired(SUB, ISS, AUD);

    let err = decoder.decode(&token).await.unwrap_err();
    assert!(
        matches!(err, AuthError::TokenExpired),
        "expected TokenExpired, got: {err}"
    );
}

#[tokio::test]
async fn wrong_issuer_returns_invalid_issuer_error() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&default_config(), cache, OidcClaimsExtractor::default());

    let token = TokenFactory::new(&key).wrong_issuer(SUB, AUD);
    let err = decoder.decode(&token).await.unwrap_err();

    assert!(
        matches!(err, AuthError::InvalidIssuer),
        "expected InvalidIssuer, got: {err}"
    );
}

#[tokio::test]
async fn wrong_audience_returns_invalid_audience_error() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&default_config(), cache, OidcClaimsExtractor::default());

    let token = TokenFactory::new(&key).wrong_audience(SUB, ISS);
    let err = decoder.decode(&token).await.unwrap_err();

    assert!(
        matches!(err, AuthError::InvalidAudience),
        "expected InvalidAudience, got: {err}"
    );
}

#[tokio::test]
async fn audience_validation_disabled_accepts_any_aud() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;

    let config = AuthContextConfig {
        expected_issuer: Some(ISS.to_owned()),
        expected_audience: None,
        ..Default::default()
    };

    let decoder = JwtDecoder::new(&config, cache, OidcClaimsExtractor::default());
    let token = TokenFactory::new(&key).valid(SUB, ISS, "completely-different-audience", "openid");

    decoder.decode(&token).await.unwrap();
}

#[tokio::test]
async fn clock_skew_tolerates_slight_expiry() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;

    // expired() sets exp = now - 60; leeway of 120 s covers it.
    let config = AuthContextConfig {
        clock_skew: Duration::from_secs(120),
        expected_issuer: Some(ISS.to_owned()),
        expected_audience: Some(AUD.to_owned()),
        ..Default::default()
    };

    let decoder = JwtDecoder::new(&config, cache, OidcClaimsExtractor::default());
    let token = TokenFactory::new(&key).expired(SUB, ISS, AUD);

    decoder.decode(&token).await.unwrap();
}
