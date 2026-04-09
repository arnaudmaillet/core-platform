#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_phone_number::{
        VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{Email, ExternalId, PhoneNumber};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_verify_phone_success() {
        let f = TestFixture::new(VerifyPhoneNumberUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let phone = PhoneNumber::try_new("+33612345678").unwrap();

        // 1. Arrange : Compte avec téléphone non vérifié (Version 1)
        let identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_555"),
        )
        .with_phone(phone)
        .build();

        assert!(!identity.is_phone_verified());
        f.identity_repo().insert(identity);

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            code: "123456".into(),
        };

        // 2. Act : On récupère l'Account mis à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(
            result.is_ok(),
            "La vérification du téléphone devrait réussir"
        );
        let updated = result.unwrap();

        assert!(updated.is_phone_verified());
        assert_eq!(updated.version(), 2, "La version doit être passée à 2");

        // 4. Persistence réelle
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.is_phone_verified());

        // 5. Outbox
        assert_eq!(
            f.outbox_count(),
            1,
            "Un événement PhoneVerified attendu"
        );
    }

    #[tokio::test]
    async fn test_verify_phone_idempotency() {
        let f = TestFixture::new(VerifyPhoneNumberUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let code = "000000";

        // 1. Arrange : On simule un téléphone déjà vérifié (Version 2)
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("s@test.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .with_phone(PhoneNumber::try_new("+33600000000").unwrap())
        .build();

        identity.verify_phone(&code).unwrap();
        identity.pull_events();
        let version_verified = identity.version();

        f.identity_repo().insert(identity);

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            code: code.to_string(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_phone_verified());
        assert_eq!(
            returned.version(),
            version_verified,
            "La version ne doit pas augmenter"
        );

        // 4. Outbox
        assert_eq!(
            f.outbox_count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(VerifyPhoneNumberUseCase::new);
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

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            code: "000000".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
