// crates/auth/src/application/interceptor.rs

use shared_kernel::{security::JwtToken, types::Region};
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

        let region_str = request
            .metadata()
            .get("x-region")
            .and_then(|m| m.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing region context (x-region header)"))?;

        let region = Region::try_new(region_str)
            .map_err(|_| Status::invalid_argument("Invalid region code"))?;

        let token = JwtToken::from_raw(token_str);

        match self.validator.validate(&token) {
            Ok(claims) => {
                request.extensions_mut().insert(claims);
                request.extensions_mut().insert(region);
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
