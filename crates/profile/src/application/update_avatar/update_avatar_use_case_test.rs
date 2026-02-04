#[cfg(test)]
mod tests {
    use crate::application::update_avatar::{UpdateAvatarCommand, UpdateAvatarUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{
        OutboxRepoStub, ProfileRepositoryStub, StubTxManager,
    };
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::DomainError;
    use std::sync::{Arc, Mutex};

    /// Helper pour instancier le Use Case
    fn setup(profile: Option<Profile>) -> UpdateAvatarUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        });

        UpdateAvatarUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_update_avatar_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        let use_case = setup(Some(initial_profile));
        let new_url = Url::try_from("https://cdn.com/new_avatar.png".to_string()).unwrap();

        let cmd = UpdateAvatarCommand {
            account_id,
            region,
            new_avatar_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        // Vérification via le getter
        assert_eq!(updated.avatar_url(), Some(&new_url));
        // Version : 1 (création) + 1 (update) = 2
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_avatar_idempotency() {
        // Arrange : On crée un profil qui a DÉJÀ cet avatar
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let avatar_url = Url::try_from("https://cdn.com/existing.png".to_string()).unwrap();

        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_avatar(avatar_url.clone()); // Version passe à 2

        let use_case = setup(Some(profile));

        let cmd = UpdateAvatarCommand {
            account_id,
            region,
            new_avatar_url: avatar_url, // Même URL
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile = result.unwrap();
        // La version ne doit pas avoir bougé car update_avatar a retourné false
        assert_eq!(profile.version(), 2);
    }

    #[tokio::test]
    async fn test_update_avatar_not_found() {
        // Arrange
        let use_case = setup(None);
        let url = Url::try_from("https://cdn.com/photo.png".to_string()).unwrap();

        let cmd = UpdateAvatarCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_avatar_url: url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_avatar_concurrency_conflict() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        // On configure le stub pour renvoyer un conflit au moment du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case =
            UpdateAvatarUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = UpdateAvatarCommand {
            account_id,
            region,
            new_avatar_url: Url::try_from("https://new.com".to_string()).unwrap(),
        };

        let result = use_case.execute(cmd).await;

        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_update_avatar_db_error() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        // On simule une erreur SQL interne
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("DB Down".into()))),
            ..Default::default()
        });

        let use_case =
            UpdateAvatarUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let result = use_case
            .execute(UpdateAvatarCommand {
                account_id,
                region,
                new_avatar_url: Url::try_from("https://new.com".to_string()).unwrap(),
            })
            .await;

        assert!(matches!(result, Err(DomainError::Internal(_))));
    }
}
