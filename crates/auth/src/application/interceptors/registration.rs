// crates/auth/src/application/public_registration_interceptor.rs

use crate::domain::validator::TokenValidator;
use shared_kernel::security::JwtToken;
use shared_kernel::types::Region;
use std::sync::Arc;
use tonic::{Request, Status, service::Interceptor};

#[derive(Clone)]
pub struct RegistrationInterceptor {
    validator: Arc<dyn TokenValidator>,
}

impl RegistrationInterceptor {
    pub fn new(validator: Arc<dyn TokenValidator>) -> Self {
        Self { validator }
    }
}

impl Interceptor for RegistrationInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let region_str = request
            .metadata()
            .get("x-region")
            .and_then(|m| m.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing region context (x-region header)"))?;

        let region = Region::try_new(region_str)
            .map_err(|_| Status::invalid_argument("Invalid region code"))?;
        request.extensions_mut().insert(region);

        if let Some(header_val) = request
            .metadata()
            .get("authorization")
            .and_then(|m| m.to_str().ok())
        {
            let token_str = header_val
                .strip_prefix("Bearer ")
                .ok_or_else(|| Status::unauthenticated("Malformed authorization header"))?;

            let token = JwtToken::from_raw(token_str);

            // Si un token est fourni, on VALIDE obligatoirement sa signature cryptographique
            if let Ok(claims) = self.validator.validate(&token) {
                request.extensions_mut().insert(claims);
            } else {
                return Err(Status::unauthenticated("Invalid social registration token"));
            }
        }

        // On laisse passer la requête vers le AccountRegistrationService!
        Ok(request)
    }
}
