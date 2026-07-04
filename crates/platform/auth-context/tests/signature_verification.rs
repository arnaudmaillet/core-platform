mod common;

use auth_context::{AuthContextConfig, AuthError, JwksCache, JwtDecoder, OidcClaimsExtractor};
use common::{TestKeyPair, TokenFactory};

const ISS: &str = "https://idp.example.com/realms/platform";
const AUD: &str = "platform-api";
const SUB: &str = "user-sig-test";

async fn seeded_cache(key: &TestKeyPair) -> JwksCache {
    let cache = JwksCache::new();
    cache.replace(key.as_cache_map()).await;
    cache
}

fn permissive_config() -> AuthContextConfig {
    AuthContextConfig {
        expected_issuer: Some(ISS.to_owned()),
        expected_audience: Some(AUD.to_owned()),
        ..Default::default()
    }
}

#[tokio::test]
async fn tampered_payload_returns_invalid_signature() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&permissive_config(), cache, OidcClaimsExtractor::default());
    let factory = TokenFactory::new(&key);

    let token = factory.valid(SUB, ISS, AUD, "openid");

    // Flip a character in the payload segment to break the signature.
    let mut parts: Vec<&str> = token.splitn(3, '.').collect();
    let original_payload = parts[1].to_owned();
    let tampered_payload = {
        let mut chars: Vec<char> = original_payload.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        chars.into_iter().collect::<String>()
    };
    parts[1] = &tampered_payload;
    let tampered_token = parts.join(".");

    let err = decoder.decode(&tampered_token).await.unwrap_err();
    assert!(
        matches!(
            err,
            AuthError::InvalidSignature | AuthError::MalformedToken(_)
        ),
        "expected InvalidSignature or MalformedToken, got: {err}"
    );
}

#[tokio::test]
async fn token_signed_with_unknown_key_returns_unknown_kid() {
    let registered_key = TestKeyPair::generate();
    let unknown_key = TestKeyPair::generate();

    let cache = seeded_cache(&registered_key).await;
    let decoder = JwtDecoder::new(&permissive_config(), cache, OidcClaimsExtractor::default());

    // Token signed with the unknown key — its kid is not in the cache.
    let token = TokenFactory::new(&unknown_key).valid(SUB, ISS, AUD, "openid");

    let err = decoder.decode(&token).await.unwrap_err();
    assert!(
        matches!(err, AuthError::UnknownKid(_)),
        "expected UnknownKid, got: {err}"
    );
}

#[tokio::test]
async fn completely_malformed_string_returns_malformed_token() {
    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&permissive_config(), cache, OidcClaimsExtractor::default());

    let err = decoder.decode("not.a.jwt").await.unwrap_err();
    assert!(
        matches!(err, AuthError::MalformedToken(_) | AuthError::MissingKid | AuthError::UnknownKid(_)),
        "expected a structural error variant, got: {err}"
    );
}

#[tokio::test]
async fn token_without_kid_header_returns_missing_kid() {
    // Build a token that has no kid in the header by using jsonwebtoken directly.
    use jsonwebtoken::{encode, Algorithm, Header};
    use serde_json::json;

    let key = TestKeyPair::generate();
    let cache = seeded_cache(&key).await;
    let decoder = JwtDecoder::new(&permissive_config(), cache, OidcClaimsExtractor::default());

    let header = Header::new(Algorithm::RS256); // no .kid set
    let claims = json!({
        "sub": SUB,
        "iss": ISS,
        "aud": AUD,
        "exp": 9_999_999_999i64,
    });
    let token = encode(&header, &claims, &key.encoding_key).unwrap();

    let err = decoder.decode(&token).await.unwrap_err();
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got: {err}"
    );
}

#[tokio::test]
async fn key_rotation_new_key_accepted_after_cache_replace() {
    let old_key = TestKeyPair::generate();
    let new_key = TestKeyPair::generate();

    let cache = seeded_cache(&old_key).await;
    let decoder = JwtDecoder::new(&permissive_config(), cache.clone(), OidcClaimsExtractor::default());

    // Token signed with old key works before rotation.
    let old_token = TokenFactory::new(&old_key).valid(SUB, ISS, AUD, "openid");
    decoder.decode(&old_token).await.unwrap();

    // Rotate: replace the cache with the new key set only.
    cache.replace(new_key.as_cache_map()).await;

    // Old token is now rejected (unknown kid).
    let err = decoder.decode(&old_token).await.unwrap_err();
    assert!(matches!(err, AuthError::UnknownKid(_)));

    // New token is accepted.
    let new_token = TokenFactory::new(&new_key).valid(SUB, ISS, AUD, "openid");
    decoder.decode(&new_token).await.unwrap();
}
