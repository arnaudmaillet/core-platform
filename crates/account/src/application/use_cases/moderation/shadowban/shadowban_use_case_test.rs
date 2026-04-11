#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::shadowban::{
        ShadowbanCommand, ShadowbanUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_shadowban_account_success() {
        let f = TestFixture::new(ShadowbanUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Compte sain (Version 1)
        f.metadata_repo()
            .insert(AccountMetadata::builder(account_id).build());

        let cmd = ShadowbanCommand {
            account_id,
            reason: "Spam behavior detected".into(),
        };

        // 2. Act : On récupère l'entité mise à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert!(updated.is_shadowbanned());
        assert!(
            updated
                .moderation_notes()
                .unwrap()
                .contains("Spam behavior detected")
        );
        assert_eq!(updated.version(), 2);

        // 4. Persistence
        let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.is_shadowbanned());

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::SHADOWBAN_STATUS_UPDATED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::SHADOWBAN_UPDATED.to_string())
        );
    }

    #[tokio::test]
    async fn test_shadowban_idempotency() {
        let f = TestFixture::new(ShadowbanUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Déjà banni (Version 2)
        let mut metadata = AccountMetadata::builder(account_id).build();
        metadata.shadowban("First ban".into()).unwrap();
        metadata.pull_events(); // Clear events
        let version_after_ban = metadata.version();

        f.metadata_repo().insert(metadata);

        let cmd = ShadowbanCommand {
            account_id,
            reason: "Second report".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_shadowbanned());
        assert_eq!(returned.version(), version_after_ban);

        // 4. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ShadowbanUseCase::new);
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

        let cmd = ShadowbanCommand {
            account_id,
            reason: "Second report".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
