#[cfg(test)]
mod tests {
    use crate::application::moderation::decrease_trust_score::{
        DecreaseTrustScoreCommand, DecreaseTrustScoreUseCase,
    };
    use crate::domain::account::entities::AccountMetadata;
    use crate::domain::repositories::AccountMetadataRepositoryStub;
    use crate::domain::value_objects::AccountRole;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;
    use uuid::Uuid;

    fn setup() -> (
        DecreaseTrustScoreUseCase,
        Arc<AccountMetadataRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            DecreaseTrustScoreUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_decrease_trust_score_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let now = chrono::Utc::now();

        // 1. Arrange : On RESTAURE avec un score de 100 en v1
        let metadata = AccountMetadata::restore(
            account_id.clone(),
            region.clone(),
            AccountRole::User,
            false,
            false,
            100, // On force le départ à 100
            None,
            None,
            None,
            now,
            shared_kernel::domain::events::AggregateMetadata::restore(1),
        );
        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 30,
            reason: "Suspicious activity".into(),
        };

        // 2. Act
        let updated = use_case.execute(cmd).await.unwrap();

        // 3. Assert
        assert_eq!(updated.trust_score(), 70, "100 - 30 devrait donner 70");
        assert_eq!(updated.version(), 2, "v1 + une mutation = v2");

        let saved = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_decrease_trust_score_clamping_and_shadowban() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let now = chrono::Utc::now();

        // 1. Arrange : On RESTAURE en version 1 avec un score de 20
        let metadata = AccountMetadata::restore(
            account_id.clone(),
            region.clone(),
            AccountRole::User,
            false,
            false,
            20, // Score de départ
            None,
            None,
            None,
            now,
            shared_kernel::domain::events::AggregateMetadata::restore(1),
        );
        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 50, // 20 - 50 -> tombe à 0
            reason: "Heavy violation".into(),
        };

        // 2. Act
        let updated = use_case.execute(cmd).await.unwrap();

        // 3. Assert
        assert_eq!(updated.trust_score(), 0);
        assert!(updated.is_shadowbanned());

        // IMPORTANT : Version 3 car : v1 + 1(score) + 1(shadowban) = 3
        assert_eq!(
            updated.version(),
            3,
            "La version doit être 3 car deux mutations distinctes ont eu lieu"
        );

        // 4. Vérification Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(
            events.len(),
            2,
            "Doit contenir TrustScoreAdjusted ET ShadowbanStatusChanged"
        );
    }

    #[tokio::test]
    async fn test_decrease_trust_score_idempotency_at_minimum() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let now = chrono::Utc::now();

        // --- ARRANGE ---
        // Correction de l'appel restore avec les 11 arguments
        let metadata = AccountMetadata::restore(
            account_id.clone(),
            region.clone(),
            AccountRole::User,                                            // role
            false,                                                        // is_beta_tester
            false,                                                        // is_shadowbanned
            20,   // trust_score (on le met à 20 pour tester le clamping)
            None, // last_moderation_at (Option<DateTime>)
            None, // moderation_notes (Option<String>)
            None, // estimated_ip (Option<String>)
            now,  // updated_at
            shared_kernel::domain::events::AggregateMetadata::restore(1), // metadata (Version 1)
        );

        metadata_repo.add_metadata(metadata);

        // On crée une commande pour baisser de 50.
        // Comme le score est à 20, il devrait tomber à 0 (clamping) et déclencher un shadowban.
        // MAIS ici on veut tester l'IDEMPOTENCE, donc on va mettre le score à 0 dans le restore
        // et s'assurer que si on est déjà à 0 et déjà shadowbanned, rien ne bouge.

        // RE-ARRANGE pour un vrai test d'idempotence au plancher :
        let metadata_at_floor = AccountMetadata::restore(
            account_id.clone(),
            region.clone(),
            AccountRole::User,
            false,
            true, // DEJÀ SHADOWBANNED
            0,    // DEJÀ À ZERO
            Some(now),
            Some("Initial penalty".into()),
            None,
            now,
            shared_kernel::domain::events::AggregateMetadata::restore(1),
        );
        metadata_repo.add_metadata(metadata_at_floor);

        let cmd = DecreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 10,
            reason: "Already at floor".into(),
        };

        // --- ACT ---
        let result = use_case.execute(cmd).await.unwrap();

        // --- ASSERT ---
        // Le score reste à 0, l'état shadowbanned reste true, la version reste à 1
        assert_eq!(result.trust_score(), 0);
        assert_eq!(result.is_shadowbanned(), true);
        assert_eq!(result.version(), 1);

        // Vérification Repo : l'objet en base n'a pas été modifié (pas de save)
        let saved = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(saved.version(), 1);

        // Vérification Outbox : aucun événement TrustScoreAdjusted produit
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo
            .add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            amount: 10,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard: renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        metadata_repo
            .add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        *metadata_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "DB Busy".into(),
        });

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: region,
            amount: 1,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }
}
