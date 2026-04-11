#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::change_role::{
        ChangeRoleCommand, ChangeRoleUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountRole, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_change_role_success() {
        let f = TestFixture::new(ChangeRoleUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Nouveau compte (Rôle User par défaut, Version 1)
        f.metadata_repo()
            .insert(AccountMetadata::builder(account_id).build());

        let cmd = ChangeRoleCommand {
            account_id,
            new_role: AccountRole::Moderator,
            reason: "Joined the safety team".into(),
        };

        // 2. Act : On récupère l'entité Metadata mise à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "Le changement de rôle devrait réussir");
        let updated = result.unwrap();

        assert_eq!(updated.role(), AccountRole::Moderator);
        assert!(
            updated
                .moderation_notes()
                .unwrap()
                .contains("Joined the safety team")
        );
        assert_eq!(updated.version(), 2, "La version doit passer à 2");

        // 4. Persistence
        let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.role(), AccountRole::Moderator);

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::ROLE_CHANGED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::ROLE_CHANGED.to_string())
        );
    }

    #[tokio::test]
    async fn test_change_role_idempotency() {
        let f: TestFixture<ChangeRoleUseCase> = TestFixture::new(ChangeRoleUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Déjà modérateur (Version passe à 2 lors du setup)
        let mut metadata = AccountMetadata::builder(account_id).build();
        metadata
            .change_role(AccountRole::Moderator, "init".into())
            .unwrap();
        metadata.pull_events(); // Clear events du setup
        let version_after_setup = metadata.version();

        f.metadata_repo().insert(metadata);

        let cmd = ChangeRoleCommand {
            account_id,
            new_role: AccountRole::Moderator, // On redemande Moderator
            reason: "Duplicate promotion".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(returned.role(), AccountRole::Moderator);
        assert_eq!(
            returned.version(),
            version_after_setup,
            "La version ne doit pas augmenter"
        );

        assert_eq!(
            f.outbox_repo().count(),
            0,
            "L'idempotence ne doit générer aucun événement"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ChangeRoleUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        let new_role = AccountRole::Moderator;
        let reason = "some_reason";

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = ChangeRoleCommand {
            account_id,
            new_role,
            reason: reason.to_string(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
