#[cfg(test)]
mod tests {
    use crate::{
        Claims,
        application::interceptor::AuthInterceptor,
        domain::validator::{AuthError, TokenValidator},
    };
    use shared_kernel::{
        security::JwtToken,
        types::{Region, SubId},
    };
    use std::{
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tonic::{Request, service::Interceptor};

    // --- LE MOCK DU TRAIT ---
    struct SimpleMockValidator {
        should_succeed: bool,
    }

    impl TokenValidator for SimpleMockValidator {
        fn validate(&self, _token: &JwtToken) -> Result<Claims, AuthError> {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");
            let exp = since_the_epoch.as_secs() + 3600;
            if self.should_succeed {
                Ok(Claims {
                    sub_id: SubId::from_raw("user-123"),
                    email: None,
                    email_verified: None,
                    phone_number: None,
                    realm_access: None,
                    exp,
                })
            } else {
                Err(AuthError::InvalidToken)
            }
        }
    }

    #[test]
    fn test_interceptor_extracts_token_correctly() {
        // Arrange
        let validator = Arc::new(SimpleMockValidator {
            should_succeed: true,
        });
        let mut interceptor = AuthInterceptor::new(validator);

        let mut req = Request::new(());
        let metadata = req.metadata_mut();
        metadata.insert("authorization", "Bearer valid-token".parse().unwrap());
        metadata.insert("x-region", "EU".parse().unwrap());

        // Act
        let res = interceptor.call(req);

        // Assert
        assert!(res.is_ok());
        let unwrapped_req = res.unwrap();

        let claims = unwrapped_req.extensions().get::<Claims>().cloned();
        assert_eq!(claims.unwrap().sub_id, SubId::from_raw("user-123"));

        let region = unwrapped_req.extensions().get::<Region>().cloned();
        assert!(region.is_some());
    }

    #[test]
    fn test_interceptor_returns_unauthenticated_on_failed_validation() {
        // Arrange
        let validator = Arc::new(SimpleMockValidator {
            should_succeed: false,
        });
        let mut interceptor = AuthInterceptor::new(validator);

        let mut req = Request::new(());
        let metadata = req.metadata_mut();
        metadata.insert("authorization", "Bearer bad-token".parse().unwrap());
        metadata.insert("x-region", "EU".parse().unwrap());

        // Act
        let res = interceptor.call(req);

        // Assert
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
        assert_eq!(err.message(), "Token invalide");
    }
}
