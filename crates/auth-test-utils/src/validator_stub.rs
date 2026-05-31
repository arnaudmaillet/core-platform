// crates/auth-test-utils/src/mock_validator.rs

use auth::{AuthError, Claims, TokenValidator};
use shared_kernel::security::JwtToken;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct TokenValidatorStub {
    stubbed_tokens: RwLock<HashMap<String, Claims>>,
    should_fail_with: RwLock<Option<AuthError>>,
}

impl TokenValidatorStub {
    pub fn new() -> Self {
        Self {
            stubbed_tokens: RwLock::new(HashMap::new()),
            should_fail_with: RwLock::new(None),
        }
    }

    /// 🛠️ Helper pour tes scénarios de test : associe un token brut à des Claims spécifiques
    pub fn stub_token(&self, token_raw: &str, claims: Claims) {
        let mut tokens = self.stubbed_tokens.write().unwrap();
        tokens.insert(token_raw.to_string(), claims);
    }

    /// 🛠️ Helper pour simuler un crash ou une expiration à la volée
    pub fn force_failure(&self, error: AuthError) {
        let mut failure = self.should_fail_with.write().unwrap();
        *failure = Some(error);
    }

    pub fn clear(&self) {
        self.stubbed_tokens.write().unwrap().clear();
        *self.should_fail_with.write().unwrap() = None;
    }
}

impl TokenValidator for TokenValidatorStub {
    fn validate(&self, token: &JwtToken) -> Result<Claims, AuthError> {
        // 1. Est-ce qu'on a forcé un état d'erreur pour le test ?
        if let Some(forced_error) = self.should_fail_with.read().unwrap().as_ref().cloned() {
            return Err(forced_error);
        }

        // 2. Recherche du token en mémoire
        let tokens = self.stubbed_tokens.read().unwrap();
        if let Some(claims) = tokens.get(token.as_str()) {
            Ok(claims.clone())
        } else {
            // Si le token n'est pas enregistré dans le Stub, on le rejette poliment
            Err(AuthError::InvalidToken)
        }
    }
}
