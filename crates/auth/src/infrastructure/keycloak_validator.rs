// crates/auth/src/infrastructure/keycloak_validator.rs

use jsonwebtoken::{DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use shared_kernel::security::JwtToken;

use crate::{
    Claims,
    domain::validator::{AuthError, TokenValidator},
};

pub struct KeycloakValidator {
    jwks: JwkSet,
    validation: Validation,
}

impl KeycloakValidator {
    pub async fn new(keycloak_url: &str, realm: &str, audience: String) -> Result<Self, AuthError> {
        let jwks_url = format!(
            "{}/realms/{}/protocol/openid-connect/certs",
            keycloak_url, realm
        );

        let issuer = format!("{}/realms/{}", keycloak_url.trim_end_matches('/'), realm);

        let jwks = reqwest::get(jwks_url)
            .await
            .map_err(|_| AuthError::DiscoveryFailed)?
            .json::<JwkSet>()
            .await
            .map_err(|_| AuthError::DiscoveryFailed)?;

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);

        validation.set_audience(&[audience]);
        validation.set_issuer(&[issuer]);
        validation.validate_aud = true;

        Ok(Self { jwks, validation })
    }

    pub fn new_mock(jwks: JwkSet, validation: Validation) -> Self {
        Self { jwks, validation }
    }
}

impl TokenValidator for KeycloakValidator {
    fn validate(&self, token: &JwtToken) -> Result<Claims, AuthError> {
        let header = decode_header(token.as_str()).map_err(|_| AuthError::InvalidToken)?;
        let kid = header.kid.ok_or(AuthError::InvalidToken)?;

        if let Some(jwk) = self.jwks.find(&kid) {
            let key = DecodingKey::from_jwk(jwk).map_err(|_| AuthError::InvalidToken)?;

            let data =
                decode::<Claims>(token.as_str(), &key, &self.validation).map_err(|e| {
                    match e.kind() {
                        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                        _ => AuthError::InvalidToken,
                    }
                })?;

            Ok(data.claims)
        } else {
            Err(AuthError::InvalidToken)
        }
    }
}
