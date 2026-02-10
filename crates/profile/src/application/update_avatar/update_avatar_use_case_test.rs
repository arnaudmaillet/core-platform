// crates/profile/src/application/update_avatar/update_avatar_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
    use shared_kernel::errors::DomainError;

    use crate::application::update_avatar::{UpdateAvatarCommand, UpdateAvatarUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId};
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour instancier le Use Case avec ses dépendances
    fn setup(profile: Option<Profile>) -> UpdateAvatarUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateAvatarUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_avatar_success() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone();
        let use_case = setup(Some(initial_profile));
        let new_url = Url::try_new("https://cdn.com/new_avatar.png").unwrap();

        let cmd = UpdateAvatarCommand {
            profile_id: profile_id.clone(), // Pivot par profile_id
            region: region.clone(),
            new_avatar_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(updated.avatar_url(), Some(&new_url));
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_avatar_fails_on_region_mismatch() {
        // Arrange : Profil en EU, Commande en US
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id,
            actual_region,
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));
        let new_url = Url::try_new("https://cdn.com/new_avatar.png").unwrap();

        let cmd = UpdateAvatarCommand {
            profile_id,
            region: wrong_region,
            new_avatar_url: new_url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_avatar_idempotency() {
        // Arrange : Profil qui a DÉJÀ cet avatar
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let avatar_url = Url::try_new("https://cdn.com/existing.png").unwrap();

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();
        profile.update_avatar(&region, avatar_url.clone()).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateAvatarCommand {
            profile_id,
            region,
            new_avatar_url: avatar_url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let returned_profile = result.unwrap();
        assert_eq!(returned_profile.version(), 2);
    }

    #[tokio::test]
    async fn test_update_avatar_not_found() {
        // Arrange
        let use_case = setup(None);
        let url = Url::try_new("https://cdn.com/photo.png").unwrap();

        let cmd = UpdateAvatarCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_avatar_url: url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_avatar_concurrency_conflict() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdateAvatarUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = UpdateAvatarCommand {
            profile_id,
            region,
            new_avatar_url: Url::try_new("https://new.com/avatar.png").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_update_avatar_db_error() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("DB Down".into()))),
            ..Default::default()
        });

        let use_case = UpdateAvatarUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = UpdateAvatarCommand {
            profile_id,
            region,
            new_avatar_url: Url::try_new("https://new.com/avatar.png").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Internal(_))));
    }
}