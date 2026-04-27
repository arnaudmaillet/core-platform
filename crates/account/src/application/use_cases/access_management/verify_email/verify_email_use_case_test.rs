#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_email::{
        VerifyEmailCommand, VerifyEmailHandler
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::Account;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::ExternalId;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_verify_email_success() {
        // Le fixture fournit maintenant le Bus et l'AccountAppContext
        let f = TestFixture::new(); 
        let account_id = f.account_id();
        let cmd = VerifyEmailCommand {
            account_id,
            token: "valid_token".into(),
        };

        // 1. Arrange : On crée un agrégat complet via le Builder
        let account = Account::builder(
            account_id,
            f.region(),
            "verify@test.com".parse().unwrap(),
            ExternalId::from_raw("ext_555"),
        ).build();
        
        f.account_repo().insert(account.unwrap()); // Le stub stocke l'agrégat

        // 2. Act : On passe par le BUS
        let result = f.bus().execute(f.account_ctx(), cmd, VerifyEmailHandler).await;

        // 3. Assert
        assert!(result.is_ok(), "Le handler devrait réussir");

        // 4. Persistence : On vérifie l'état final dans le repo
        let saved = f.account_repo()
            .find_by_id(&account_id, None)
            .await
            .unwrap()
            .expect("Le compte doit exister");

        assert!(saved.identity().is_email_verified());
        assert_eq!(saved.metadata().version(), 2, "La version doit avoir été incrémentée");

        // 5. Outbox
        assert!(f.outbox_events().contains(&AccountEvent::EMAIL_VERIFIED.to_string()));
    }

    #[tokio::test]
    async fn test_verify_email_concurrency_retry() {
        let f = TestFixture::new();
        let account_id = f.account_id();
        
        // Arrange : On simule un conflit de version au premier appel
        // On peut configurer le Stub pour renvoyer une ConcurrencyConflict une fois
        f.account_repo().set_error_once(DomainError::ConcurrencyConflict { 
            reason: "Simulated conflict".into() 
        });

        let account = Account::builder(account_id, f.region(), "retry@test.com".parse().unwrap(), "ext".parse().unwrap()).build();
        f.account_repo().insert(account.unwrap());

        let cmd = VerifyEmailCommand { account_id, token: "token".into() };

        // Act
        let result = f.bus().execute(f.account_ctx(), cmd, VerifyEmailHandler).await;

        // Assert
        assert!(result.is_ok(), "Le bus doit avoir retenté l'opération avec succès");
        assert_eq!(f.account_repo().find_direct(&account_id).unwrap().metadata().version(), 2);
    }   


    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new();
        let account_id = f.account_id();
        let wrong_region = "us".parse().unwrap();

        // Compte en base sur la mauvaise région
        let account = Account::builder(account_id, wrong_region, "hacker@test.com".parse().unwrap(), "ext".parse().unwrap()).build();
        f.account_repo().insert(account.unwrap());

        let cmd = VerifyEmailCommand { account_id, token: "token".into() };

        // Act
        let result = f.bus().execute(f.account_ctx(), cmd, VerifyEmailHandler).await;

        // Assert : La sécurité est gérée par ctx.account() qui renvoie NotFound si la région ne matche pas
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }   
}
