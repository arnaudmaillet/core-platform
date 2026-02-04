// crates/profile/src/application/update_stats/update_stats_use_case_test.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::update_stats::{UpdateStatsCommand, UpdateStatsUseCase};
    use crate::domain::repositories::ProfileStatsRepository;
    use crate::utils::ProfileStatsRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_update_stats_nominal_increment() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let repo = Arc::new(ProfileStatsRepositoryStub::default());
        let use_case = UpdateStatsUseCase::new(repo.clone());

        let cmd = UpdateStatsCommand {
            account_id: account_id.clone(),
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
            .find_by_id(&account_id, &region)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stats.follower_count(), 5);
        assert_eq!(stats.following_count(), 2);
    }

    #[tokio::test]
    async fn test_update_stats_nominal_decrement() {
        // Arrange : On part d'un état à 10 followers
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let repo = Arc::new(ProfileStatsRepositoryStub::default());

        // On initialise le stub
        repo.save(&account_id, &region, 10, 0, 0)
            .await
            .unwrap();

        let use_case = UpdateStatsUseCase::new(repo.clone());
        let cmd = UpdateStatsCommand {
            account_id: account_id.clone(),
            region: region.clone(),
            follower_delta: -3, // On perd 3 followers
            following_delta: 0,
            post_delta: 0,
        };

        // Act
        use_case.execute(cmd).await.unwrap();

        // Assert
        let stats = repo
            .find_by_id(&account_id, &region)
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
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
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
        // Arrange : Le repo échoue 10 fois (le défaut de RetryConfig est souvent 3 ou 5)
        let repo = Arc::new(ProfileStatsRepositoryStub::default());
        *repo.fail_count.lock().unwrap() = 10;

        let use_case = UpdateStatsUseCase::new(repo.clone());
        let cmd = UpdateStatsCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            follower_delta: 1,
            following_delta: 0,
            post_delta: 0,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(
            result.is_err(),
            "Le Use Case doit finir par échouer si Scylla est définitivement HS"
        );
    }
}
