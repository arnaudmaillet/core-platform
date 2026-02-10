// crates/profile/src/application/update_stats/update_stats_use_case_test.rs

#[cfg(test)]
mod tests {
    use crate::application::update_stats::{UpdateStatsCommand, UpdateStatsUseCase};
    use crate::domain::repositories::{ProfileStatsRepository, ProfileStatsRepositoryStub};
    use crate::domain::value_objects::ProfileId; // Ajout du ProfileId
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_update_stats_nominal_increment() {
        // Arrange
        let profile_id = ProfileId::new(); // Pivot sur le profil
        let region = RegionCode::try_new("eu").unwrap();
        let repo = Arc::new(ProfileStatsRepositoryStub::default());
        let use_case = UpdateStatsUseCase::new(repo.clone());

        let cmd = UpdateStatsCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            follower_delta: 5,
            following_delta: 2,
            post_delta: 1,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let stats = repo
            .fetch(&profile_id, &region)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stats.follower_count(), 5);
        assert_eq!(stats.following_count(), 2);
    }

    #[tokio::test]
    async fn test_update_stats_nominal_decrement() {
        // Arrange : On part d'un état à 10 followers
        let profile_id = ProfileId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let repo = Arc::new(ProfileStatsRepositoryStub::default());

        // On initialise le stub (save utilise profile_id + region)
        repo.save(&profile_id, &region, 10, 0, 0)
            .await
            .unwrap();

        let use_case = UpdateStatsUseCase::new(repo.clone());
        let cmd = UpdateStatsCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            follower_delta: -3, // On perd 3 followers
            following_delta: 0,
            post_delta: 0,
        };

        // Act
        use_case.execute(cmd).await.unwrap();

        // Assert
        let stats = repo
            .fetch(&profile_id, &region)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stats.follower_count(), 7);
    }

    #[tokio::test]
    async fn test_update_stats_resilience_with_retry() {
        // Arrange : Le repo va échouer 2 fois puis réussir
        let repo = Arc::new(ProfileStatsRepositoryStub::default());
        *repo.fail_count.lock().unwrap() = 2;

        let use_case = UpdateStatsUseCase::new(repo.clone());
        let cmd = UpdateStatsCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            follower_delta: 1,
            following_delta: 0,
            post_delta: 0,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(
            result.is_ok(),
            "Le Use Case aurait dû réussir après 2 retries"
        );
        assert_eq!(
            *repo.fail_count.lock().unwrap(),
            0,
            "Toutes les tentatives d'échec ont dû être consommées"
        );
    }

    #[tokio::test]
    async fn test_update_stats_failure_after_max_retries() {
        // Arrange : Le repo échoue 10 fois
        let repo = Arc::new(ProfileStatsRepositoryStub::default());
        *repo.fail_count.lock().unwrap() = 10;

        let use_case = UpdateStatsUseCase::new(repo.clone());
        let cmd = UpdateStatsCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            follower_delta: 1,
            following_delta: 0,
            post_delta: 0,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(
            result.is_err(),
            "Le Use Case doit finir par échouer si ScyllaDB est définitivement HS"
        );
    }
}