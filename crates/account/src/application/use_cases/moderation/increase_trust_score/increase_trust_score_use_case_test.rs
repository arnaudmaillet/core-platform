#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::increase_trust_score::{
        IncreaseTrustScoreCommand, IncreaseTrustScoreUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_increase_trust_score_success() {
        let f = TestFixture::new(IncreaseTrustScoreUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Score par défaut (ex: 50)
        f.metadata_repo().insert(
            AccountMetadata::builder(account_id)
                .with_trust_score(50)
                .build(),
        );

        let cmd = IncreaseTrustScoreCommand {
            account_id: account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 20, // 50 + 20 = 70
            reason: "Email verified".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let updated = result.unwrap();

        // 3. Assert
        assert_eq!(updated.trust_score(), 70);
        assert_eq!(updated.version(), 2);

       let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement IncreaseTrustScore attendu"
        );
    }

    #[tokio::test]
    async fn test_increase_trust_score_cap_at_one_hundred() {
        let f = TestFixture::new(IncreaseTrustScoreUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : On monte manuellement à 90
        let mut metadata = AccountMetadata::builder(account_id)
            .with_trust_score(50)
            .build();
        metadata
            .increase_trust_score(uuid::Uuid::now_v7(), 40, "bump".into())
            .unwrap();
        f.metadata_repo().insert(metadata);

        let cmd = IncreaseTrustScoreCommand {
            account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 50, // 90 + 50 -> Doit saturer à 100
            reason: "High activity".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let updated = result.unwrap();

        // 3. Assert
        assert_eq!(
            updated.trust_score(),
            100,
            "Le score ne doit pas dépasser 100"
        );
        assert_eq!(
            updated.version(),
            3,
            "La version doit avoir augmenté (90 -> 100 est un changement)"
        );


       let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.trust_score(), 100);
    }

    #[tokio::test]
    async fn test_increase_trust_score_idempotency_at_max() {
        let f = TestFixture::new(IncreaseTrustScoreUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Déjà au max (100)
        let mut metadata = AccountMetadata::builder(account_id).build();
        metadata
            .increase_trust_score(uuid::Uuid::now_v7(), 100, "max out".into())
            .unwrap();
        metadata.pull_events(); // On vide les événements du setup
        let version_at_max = metadata.version();

        f.metadata_repo().insert(metadata);

        let cmd = IncreaseTrustScoreCommand {
            account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 10,
            reason: "Should do nothing".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let returned = result.unwrap();

        // 3. Assert
        assert_eq!(returned.trust_score(), 100);
        assert_eq!(
            returned.version(),
            version_at_max,
            "La version ne doit pas bouger"
        );

        // 4. Assert : Aucun événement produit
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(IncreaseTrustScoreUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );
        let cmd = IncreaseTrustScoreCommand {
            account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 10,
            reason: "Should do nothing".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
