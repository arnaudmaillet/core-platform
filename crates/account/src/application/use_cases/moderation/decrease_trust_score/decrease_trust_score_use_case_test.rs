#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::decrease_trust_score::{
        DecreaseTrustScoreCommand, DecreaseTrustScoreUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountMetadata};
    use crate::domain::value_objects::{AccountRole, Email, ExternalId};
    use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_decrease_trust_score_success() {
        let f = TestFixture::new(DecreaseTrustScoreUseCase::new);
        let account_id = f.account_id();
        let now = chrono::Utc::now();

        // 1. Arrange : On RESTAURE avec un score de 100 en v1
        let metadata = AccountMetadata::restore(
            account_id,
            AccountRole::User,
            false,
            false,
            100, // On force le départ à 100
            None,
            None,
            None,
            now,
            AggregateMetadata::restore(1),
        );
        f.metadata_repo().insert(metadata);

        let cmd = DecreaseTrustScoreCommand {
            account_id: account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 30,
            reason: "Suspicious activity".into(),
        };

        // 2. Act
        let updated = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        // 3. Assert
        assert_eq!(updated.trust_score(), 70, "100 - 30 devrait donner 70");
        assert_eq!(updated.version(), 2, "v1 + une mutation = v2");

       let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement DecreaseTrustScore attendu"
        );
    }

    #[tokio::test]
    async fn test_decrease_trust_score_clamping_and_shadowban() {
        let f = TestFixture::new(DecreaseTrustScoreUseCase::new);
        let account_id = f.account_id();
        let now = chrono::Utc::now();

        // 1. Arrange : On RESTAURE en version 1 avec un score de 20
        let metadata = AccountMetadata::restore(
            account_id,
            AccountRole::User,
            false,
            false,
            20, // Score de départ
            None,
            None,
            None,
            now,
           AggregateMetadata::restore(1),
        );
        f.metadata_repo().insert(metadata);

        let cmd: DecreaseTrustScoreCommand = DecreaseTrustScoreCommand {
            account_id,
            action_id: uuid::Uuid::now_v7(),
            amount: 50, // 20 - 50 -> tombe à 0
            reason: "Heavy violation".into(),
        };

        // 2. Act
        let updated = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

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
        assert_eq!(
            f.outbox_repo().count(),
            2,
            "Doit contenir TrustScoreAdjusted ET ShadowbanStatusChanged"
        );
    }

    #[tokio::test]
    async fn test_decrease_trust_score_idempotency_at_minimum() {
        let f = TestFixture::new(DecreaseTrustScoreUseCase::new);
        let account_id = f.account_id();
        let now = chrono::Utc::now();

        // --- ARRANGE ---
        // Correction de l'appel restore avec les 11 arguments
        let metadata = AccountMetadata::restore(
            account_id,
            AccountRole::User,
            false,
            false,
            20,
            None,
            None,
            None,
            now,
            AggregateMetadata::restore(1),
        );

        f.metadata_repo().insert(metadata);

        // On crée une commande pour baisser de 50.
        // Comme le score est à 20, il devrait tomber à 0 (clamping) et déclencher un shadowban.
        // MAIS ici on veut tester l'IDEMPOTENCE, donc on va mettre le score à 0 dans le restore
        // et s'assurer que si on est déjà à 0 et déjà shadowbanned, rien ne bouge.

        // RE-ARRANGE pour un vrai test d'idempotence au plancher :
        let metadata_at_floor = AccountMetadata::restore(
            account_id,
            AccountRole::User,
            false,
            true, // DEJÀ SHADOWBANNED
            0,    // DEJÀ À ZERO
            Some(now),
            Some("Initial penalty".into()),
            None,
            now,
            AggregateMetadata::restore(1),
        );
        f.metadata_repo().insert(metadata_at_floor);

        let cmd = DecreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id,
            amount: 10,
            reason: "Already at floor".into(),
        };

        // --- ACT ---
        let result = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        // --- ASSERT ---
        // Le score reste à 0, l'état shadowbanned reste true, la version reste à 1
        assert_eq!(result.trust_score(), 0);
        assert_eq!(result.is_shadowbanned(), true);
        assert_eq!(result.version(), 1);

        // Vérification Repo : l'objet en base n'a pas été modifié (pas de save)
       let saved = f
            .metadata_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), 1);

        // Vérification Outbox : aucun événement TrustScoreAdjusted produit
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }


    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let f = TestFixture::new(DecreaseTrustScoreUseCase::new);
        let account_id = f.account_id();

        f.metadata_repo().insert(AccountMetadata::builder(account_id).build());
        f.metadata_repo().set_error(DomainError::ConcurrencyConflict {
            reason: "DB Busy".into(),
        });

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            amount: 1,
            reason: "Test".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(DecreaseTrustScoreUseCase::new);
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
        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            amount: 1,
            reason: "Test".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
