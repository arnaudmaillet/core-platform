#[cfg(test)]
mod tests {
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::errors::DomainError;
    use crate::application::use_cases::lifecycle::upgrade_role::{UpgradeRoleCommand, UpgradeRoleUseCase};
    use crate::domain::value_objects::{AccountRole, Email, ExternalId};

    #[tokio::test]
    async fn test_upgrade_role_success() {
        let f = TestFixture::new(UpgradeRoleUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Nouveau compte (Rôle User par défaut, Version 1)
        f.metadata_repo().add_metadata(
            AccountMetadata::builder(account_id).build()
        );

        let cmd = UpgradeRoleCommand {
            account_id,
            new_role: AccountRole::Moderator,
            reason: "Joined the safety team".into(),
        };

        // 2. Act : On récupère l'entité Metadata mise à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "L'upgrade de rôle devrait réussir");
        let updated = result.unwrap();

        assert_eq!(updated.role(), AccountRole::Moderator);
        assert!(updated.moderation_notes().unwrap().contains("Joined the safety team"));
        assert_eq!(updated.version(), 2, "La version doit passer à 2");

        // 4. Persistence
        let saved = f.metadata_repo().find_by_id(&account_id).expect("Should exist");
        assert_eq!(saved.role(), AccountRole::Moderator);
        
        // 5. Outbox
        assert_eq!(f.outbox_count(), 1, "Un événement RoleUpgraded attendu");
    }

    #[tokio::test]
    async fn test_upgrade_role_idempotency() {
        let f: TestFixture<UpgradeRoleUseCase> = TestFixture::new(UpgradeRoleUseCase::new);
        let account_id = f.account_id();

        // 1. Arrange : Déjà modérateur (Version passe à 2 lors du setup)
        let mut metadata = AccountMetadata::builder(account_id).build();
        metadata.upgrade_role(AccountRole::Moderator, "init".into()).unwrap();
        metadata.pull_events(); // Clear events du setup
        let version_after_setup = metadata.version();
        
        f.metadata_repo().add_metadata(metadata);

        let cmd = UpgradeRoleCommand {
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
        assert_eq!(returned.version(), version_after_setup, "La version ne doit pas augmenter");

        assert_eq!(f.outbox_count(), 0, "L'idempotence ne doit générer aucun événement");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UpgradeRoleUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let new_role = AccountRole::Moderator;
        let reason = "some_reason";
        
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            ).build(),
        );

        let cmd = UpgradeRoleCommand {
            account_id,
            new_role,
            reason: reason.to_string()
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}