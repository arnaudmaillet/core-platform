mod common;

use std::collections::HashMap;

use auth_context::{
    ClaimsExtractor, OidcClaims, OidcClaimsExtractor, OidcExtractorConfig, RealmAccess,
    RoleSource,
};

const ISS: &str = "https://idp.example.com/realms/platform";
const AUD: &str = "platform-api";
const SUB: &str = "f47ac10b-58cc-4372-a567-0e02b2c3d479";

fn base_claims() -> OidcClaims {
    OidcClaims {
        sub: SUB.to_owned(),
        iss: Some(ISS.to_owned()),
        aud: Some(serde_json::json!(AUD)),
        exp: 9_999_999_999,
        nbf: None,
        iat: None,
        jti: None,
        scope: None,
        realm_access: None,
        resource_access: None,
        permissions: None,
        groups: None,
        tid: None,
        extra: HashMap::new(),
    }
}

#[test]
fn sub_maps_to_user_id() {
    let extractor = OidcClaimsExtractor::default();
    let principal = extractor.extract(base_claims()).unwrap();
    assert_eq!(principal.user_id.as_str(), SUB);
}

#[test]
fn empty_sub_returns_error() {
    let extractor = OidcClaimsExtractor::default();
    let mut claims = base_claims();
    claims.sub = String::new();
    assert!(matches!(
        extractor.extract(claims),
        Err(auth_context::AuthError::ClaimsExtractionFailed(_))
    ));
}

#[test]
fn scope_splits_into_permissions() {
    let extractor = OidcClaimsExtractor::default();
    let mut claims = base_claims();
    claims.scope = Some("openid profile posts:write".to_owned());

    let principal = extractor.extract(claims).unwrap();
    let perms: Vec<&str> = principal.permissions.iter().map(|p| p.as_str()).collect();

    assert!(perms.contains(&"openid"));
    assert!(perms.contains(&"profile"));
    assert!(perms.contains(&"posts:write"));
}

#[test]
fn realm_access_roles_map_to_permissions() {
    let config = OidcExtractorConfig {
        role_sources: vec![RoleSource::RealmAccessRoles],
        ..Default::default()
    };
    let extractor = OidcClaimsExtractor::new(config);
    let mut claims = base_claims();
    claims.realm_access = Some(RealmAccess {
        roles: Some(vec!["ROLE_USER".to_owned(), "ROLE_ADMIN".to_owned()]),
    });

    let principal = extractor.extract(claims).unwrap();
    assert!(principal.has_permission("ROLE_USER"));
    assert!(principal.has_permission("ROLE_ADMIN"));
}

#[test]
fn auth0_permissions_claim_maps_correctly() {
    let config = OidcExtractorConfig {
        role_sources: vec![RoleSource::PermissionsClaim],
        ..Default::default()
    };
    let extractor = OidcClaimsExtractor::new(config);
    let mut claims = base_claims();
    claims.permissions = Some(vec!["read:users".to_owned(), "write:posts".to_owned()]);

    let principal = extractor.extract(claims).unwrap();
    assert!(principal.has_permission("read:users"));
    assert!(principal.has_permission("write:posts"));
}

#[test]
fn duplicate_permissions_are_deduplicated() {
    let config = OidcExtractorConfig {
        role_sources: vec![RoleSource::Scope, RoleSource::PermissionsClaim],
        ..Default::default()
    };
    let extractor = OidcClaimsExtractor::new(config);
    let mut claims = base_claims();
    claims.scope = Some("openid posts:write".to_owned());
    claims.permissions = Some(vec!["posts:write".to_owned(), "read:users".to_owned()]);

    let principal = extractor.extract(claims).unwrap();
    let count = principal
        .permissions
        .iter()
        .filter(|p| p.as_str() == "posts:write")
        .count();

    assert_eq!(count, 1, "posts:write must appear exactly once");
}

#[test]
fn custom_role_source_reads_arbitrary_claim() {
    let config = OidcExtractorConfig {
        role_sources: vec![RoleSource::Custom("platform_roles".to_owned())],
        ..Default::default()
    };
    let extractor = OidcClaimsExtractor::new(config);
    let mut claims = base_claims();
    claims
        .extra
        .insert("platform_roles".to_owned(), serde_json::json!(["geo:admin", "social:mod"]));

    let principal = extractor.extract(claims).unwrap();
    assert!(principal.has_permission("geo:admin"));
    assert!(principal.has_permission("social:mod"));
}

#[test]
fn tenant_id_resolved_from_tid_claim() {
    let extractor = OidcClaimsExtractor::default();
    let mut claims = base_claims();
    claims.tid = Some("tenant-acme".to_owned());

    let principal = extractor.extract(claims).unwrap();
    assert_eq!(principal.tenant_id.as_deref(), Some("tenant-acme"));
}

#[test]
fn tenant_id_resolved_from_custom_claim_key() {
    let config = OidcExtractorConfig {
        tenant_id_claim: "org_id".to_owned(),
        ..Default::default()
    };
    let extractor = OidcClaimsExtractor::new(config);
    let mut claims = base_claims();
    claims
        .extra
        .insert("org_id".to_owned(), serde_json::json!("org-xyz"));

    let principal = extractor.extract(claims).unwrap();
    assert_eq!(principal.tenant_id.as_deref(), Some("org-xyz"));
}

#[test]
fn has_all_permissions_returns_false_when_one_missing() {
    let extractor = OidcClaimsExtractor::default();
    let mut claims = base_claims();
    claims.scope = Some("openid posts:read".to_owned());

    let principal = extractor.extract(claims).unwrap();
    assert!(!principal.has_all_permissions(&["openid", "posts:write"]));
}

#[test]
fn has_any_permission_returns_true_when_one_matches() {
    let extractor = OidcClaimsExtractor::default();
    let mut claims = base_claims();
    claims.scope = Some("openid".to_owned());

    let principal = extractor.extract(claims).unwrap();
    assert!(principal.has_any_permission(&["posts:write", "openid"]));
}
