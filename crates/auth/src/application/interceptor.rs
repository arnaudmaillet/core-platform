use shared_kernel::domain::value_objects::JwtToken;
use std::sync::Arc;
use tonic::{Request, Status, service::Interceptor};

use crate::domain::validator::{AuthError, TokenValidator};

#[derive(Clone)]
pub struct AuthInterceptor {
    validator: Arc<dyn TokenValidator>,
}

impl AuthInterceptor {
    pub fn new(validator: Arc<dyn TokenValidator>) -> Self {
        Self { validator }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let token_str = request
            .metadata()
            .get("authorization")
            .and_then(|m| m.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("Missing or malformed authorization header"))?;

        // On utilise from_raw ici car si le token est malformé,
        // le validator.validate() s'en occupera de toute façon.
        let token = JwtToken::from_raw(token_str);

        match self.validator.validate(&token) {
            Ok(claims) => {
                request.extensions_mut().insert(claims);
                Ok(request)
            }
            Err(e) => match e {
                AuthError::InvalidToken => Err(Status::unauthenticated("Token invalide")),
                AuthError::DiscoveryFailed => {
                    Err(Status::internal("Erreur serveur d'authentification"))
                }
                AuthError::Expired => Err(Status::unauthenticated("Token expiré")),
            },
        }
    }
}
