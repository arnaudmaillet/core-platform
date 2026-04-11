#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::set_as_beta::{
        SetAsBetaCommand, SetAsBetaUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_set_beta_status_to_true_success() {
        let f = TestFixture::new(SetAsBetaUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Nouveau compte (beta_tester = false par défaut, version 1)
        f.metadata_repo()
            .insert(AccountMetadata::builder(account_id).build());

        let cmd = SetAsBetaCommand {
            account_id,
            status: true,
            reason: "Early adopter program".into(),
        };

        // 2. Act : On s'attend à recevoir l'entité mise à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert!(updated.is_beta_tester());
        assert!(
            updated
                .moderation_notes()
                .unwrap()
                .contains("Early adopter program")
        );
        assert_eq!(updated.version(), 2);

        // 4. Persistence
        let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.is_beta_tester());

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::BETA_STATUS_UPADTED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::BETA_STATUS_UPADTED.to_string())
        );
    }

    #[tokio::test]
    async fn test_set_beta_status_idempotency() {
        let f = TestFixture::new(SetAsBetaUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : On le passe beta manuellement (Version passe à 2)
        let mut metadata = AccountMetadata::builder(account_id).build();
        metadata
            .set_beta_status(true, "initial activation".into())
            .unwrap();
        metadata.pull_events(); // On vide les events de l'initialisation
        let version_after_setup = metadata.version();

        f.metadata_repo().insert(metadata);

        let cmd = SetAsBetaCommand {
            account_id,
            status: true, // On demande encore true
            reason: "Double call".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_beta_tester());
        assert_eq!(returned.version(), version_after_setup);

        // 4. Outbox
        assert_eq!(f.outbox_repo().count(), 0, "Aucun evewnement attendu");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(SetAsBetaUseCase::new);
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

        let cmd = SetAsBetaCommand {
            account_id,
            status: true,
            reason: "Double call".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
