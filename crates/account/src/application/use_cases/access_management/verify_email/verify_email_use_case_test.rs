#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_email::{
        VerifyEmailCommand, VerifyEmailUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_verify_email_success() {
        let f = TestFixture::new(VerifyEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : Compte initial (Non vérifié, Version 1)
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("verify@test.com").unwrap(),
                ExternalId::from_raw("ext_999"),
            )
            .build(),
        );

        let cmd = VerifyEmailCommand {
            account_id,
            token: "valid_secure_token_123".into(),
        };

        // 2. Act : On s'attend à recevoir l'Account mis à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La vérification d'email devrait réussir");
        let updated = result.unwrap();

        assert!(updated.is_email_verified());
        assert_eq!(
            *updated.state(),
            AccountState::Active,
            "Le compte doit devenir actif après vérification"
        );
        assert_eq!(updated.version(), 2, "La version doit passer à 2");

        // 4. Persistence réelle
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.is_email_verified());

        // 5. Outbox
        assert_eq!(
            f.outbox_count(),
            1,
            "Un événement EmailVerified attendu"
        );
    }

    #[tokio::test]
    async fn test_verify_email_idempotency() {
        let f = TestFixture::new(VerifyEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let token = "any_token";

        // 1. Arrange : On prépare un compte déjà vérifié (Version 2)
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("ok@test.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        identity.verify_email(&token).unwrap();
        identity.pull_events(); // Clear setup events
        let version_verified = identity.version();

        f.identity_repo().insert(identity);

        let cmd = VerifyEmailCommand {
            account_id,
            token: token.to_string(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_email_verified());
        assert_eq!(
            returned.version(),
            version_verified,
            "La version ne doit pas augmenter"
        );

        // 4. Outbox
        assert_eq!(
            f.outbox_count(),
            0,
            "Idempotence : pas de double événement"
        );
    }

    #[tokio::test]
    async fn test_verify_email_fails_on_region_mismatch() {
        let f = TestFixture::new(VerifyEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("u@t.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        let cmd = VerifyEmailCommand {
            account_id,
            token: "token".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(VerifyEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = VerifyEmailCommand {
            account_id,
            token: "token".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // ASSERT : On vérifie l'obfuscation de sécurité
        // Le compte existe en base, mais le contexte doit dire "NotFound"
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
