// crates/auth/src/domain/validator_test.rs
#[cfg(test)]
mod tests {
    use crate::domain::claims::RealmAccess;
    use crate::domain::validator::{AuthError, TokenValidator};
    use crate::{Claims, KeycloakValidator};
    use jsonwebtoken::{EncodingKey, Header, Validation, encode};
    use shared_kernel::security::JwtToken;
    use shared_kernel::types::{Email, SubId};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Helper pour créer des Claims valides sans Default::default()
    fn create_test_claims(sub: &str, email: &str) -> Claims {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let exp = since_the_epoch.as_secs() + 3600;

        Claims {
            sub_id: SubId::from_raw(sub),
            email: Some(Email::from_raw(email)),
            email_verified: Some(true),
            phone_number: None,
            realm_access: Some(RealmAccess {
                roles: vec!["user".to_string()],
            }),
            exp,
        }
    }

    fn setup_mock_validator() -> (KeycloakValidator, Vec<u8>, String) {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        use jsonwebtoken::jwk::{Jwk, JwkSet, KeyAlgorithm, PublicKeyUse, RSAKeyParameters};
        use openssl::rsa::Rsa;

        let rsa = Rsa::generate(2048).unwrap();
        let private_key = rsa.private_key_to_pem().unwrap();
        let kid = "test-key-id".to_string();

        let jwk = Jwk {
            common: jsonwebtoken::jwk::CommonParameters {
                public_key_use: Some(PublicKeyUse::Signature),
                key_algorithm: Some(KeyAlgorithm::RS256),
                key_id: Some(kid.clone()),
                ..Default::default()
            },
            algorithm: jsonwebtoken::jwk::AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: jsonwebtoken::jwk::RSAKeyType::RSA,
                n: URL_SAFE_NO_PAD.encode(rsa.n().to_vec()),
                e: URL_SAFE_NO_PAD.encode(rsa.e().to_vec()),
            }),
        };

        let jwks = JwkSet { keys: vec![jwk] };
        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_aud = false;

        (
            KeycloakValidator::new_mock(jwks, validation),
            private_key,
            kid,
        )
    }

    #[test]
    fn test_security_reject_wrong_signature() {
        let (validator, _, kid) = setup_mock_validator();
        let (_, other_private_key, _) = setup_mock_validator(); // Une autre clé

        let claims = create_test_claims("user-1", "test@audit.com");

        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(kid);

        // Signé avec la MAUVAISE clé
        let token_str = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(&other_private_key).unwrap(),
        )
        .unwrap();

        let result = validator.validate(&JwtToken::from_raw(token_str));
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[test]
    fn test_domain_mapping_integrity() {
        let (validator, private_key, kid) = setup_mock_validator();

        let claims = create_test_claims("user-unique-123", "audit@secure.com");

        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(kid);

        let token_str = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(&private_key).unwrap(),
        )
        .unwrap();

        // Act
        let result = validator
            .validate(&JwtToken::from_raw(token_str))
            .expect("Should be valid");

        // Assert: Vérification de la reconstruction des Value Objects
        assert_eq!(result.sub_id, SubId::from_raw("user-unique-123"));
        assert_eq!(result.email.unwrap(), Email::from_raw("audit@secure.com"));
        assert_eq!(result.email_verified, Some(true));
        assert_eq!(result.realm_access.unwrap().roles[0], "user");
    }
}
