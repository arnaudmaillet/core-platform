#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_phone_number::{
        VerifyPhoneNumberCommand, VerifyPhoneNumberHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::Account;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{ExternalId, RegistrationIdentifier, VerificationCode};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_verify_phone_success() {
        let f = TestFixture::new();
        let account_id = f.account_id();

        // 1. Arrange : Compte avec téléphone non vérifié (Version 1)
        let account = Account::builder(
            account_id,
            f.region(),
            RegistrationIdentifier::try_from_phone("+33612345678")?,
            ExternalId::try_new("ext_555")?,
        )
        .build()
        .unwrap();

        assert!(!account.is_phone_verified());
        f.account_repo().insert(account);

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            code: "123456".into(),
        };

        // 2. Act : On récupère l'Account mis à jour
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await;

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
            .account_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.is_phone_verified());

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::PHONE_VERIFIED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::PHONE_VERIFIED.to_string())
        );
    }

    #[tokio::test]
    async fn test_verify_phone_idempotency() {
        let f = TestFixture::new();
        let account_id = f.account_id();
        let code = VerificationCode::try_new("000000")?;

        // 1. Arrange : On simule un téléphone déjà vérifié (Version 2)
        let mut account = Account::builder(
            account_id,
            f.region(),
            RegistrationIdentifier::try_from_phone("+33612345678")?,
            ExternalId::try_new("ext_555")?,
        )
        .build()
        .unwrap();

        account.verify_phone(code).unwrap();
        account.pull_events();
        let version_verified = account.version();

        f.account_repo().insert(account);

        let cmd = VerifyPhoneNumberCommand { account_id, code };

        // 2. Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await;

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
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new();
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        let code = VerificationCode::try_new("000000")?;

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.account_repo().insert(
            Account::builder(
                account_id,
                wrong_region,
                RegistrationIdentifier::try_from_phone("+33612345678")?,
                ExternalId::from_raw("ext_1"),
            )
            .build()?,
        );

        let cmd = VerifyPhoneNumberCommand { account_id, code };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
