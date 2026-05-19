#[cfg(test)]
mod tests {
    use crate::domain::entities::FollowRelation;
    use crate::entities::FollowRelationBuilder;
    use crate::repositories::CounterRepository;
    use crate::test_utils::SocialTestFixture;
    use chrono::Utc;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use shared_kernel::types::{ProfileId, Region, RegionCode};
    use uuid::Uuid;

    // Helper pour générer un ProfileId forcé sur une région spécifique
    fn mock_profile_id_in_region(region: &str) -> ProfileId {
        // Version robuste : utilise le générateur de ton kernel ou mock String
        ProfileId::try_new(format!("{}:{}", region, Uuid::new_v4())).unwrap_or_else(|_| {
            // Fallback si ton ProfileId a un parsing strict différent (ex: UUID pur mais rattaché à la région par métadonnées)
            ProfileId::generate(Region::try_new(region).unwrap())
        })
    }

    // --- TESTS DU BUILDER ---

    #[test]
    fn test_builder_should_instantiate_relation_correctly() -> Result<()> {
        // Given
        let follower = ProfileId::generate(Region::default());
        let following = ProfileId::generate(Region::default());
        let custom_time = Utc::now() - chrono::Duration::days(2);

        // When
        let relation = FollowRelationBuilder::new(follower, following)
            .with_created_at(custom_time)
            .with_version(5)
            .build()?;

        // Then
        assert_eq!(relation.follower_id(), &follower);
        assert_eq!(relation.following_id(), &following);
        assert_eq!(relation.version(), 5);
        assert_eq!(relation.created_at(), custom_time);
        Ok(())
    }

    // --- TESTS DU CONTEXTE (LECTURES & CACHE STRATEGY) ---

    #[tokio::test]
    async fn test_get_counters_cache_hit_should_return_immediately() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let profile_id = fixture.target_profile_id();

        // On alimente uniquement le cache chaud (Redis)
        fixture
            .cache_counter_repo()
            .seed_counters(profile_id, 100, 50);

        // When
        let counters = fixture
            .social_ctx()
            .get_profile_counters(profile_id)
            .await?;

        // Then
        assert_eq!(counters.followers_count().value(), 100);
        assert_eq!(counters.following_count().value(), 50);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_counters_cache_miss_should_fallback_and_warm_cache() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let profile_id = fixture.target_profile_id();

        // Donnée présente uniquement dans la DB consolidée (ScyllaDB)
        fixture
            .db_counter_repo()
            .seed_counters(profile_id, 1250, 420);

        // When
        let counters = fixture
            .social_ctx()
            .get_profile_counters(profile_id)
            .await?;

        // Then
        assert_eq!(counters.followers_count().value(), 1250);
        assert_eq!(counters.following_count().value(), 420);

        // Verification du CACHE WARMING : Redis doit maintenant avoir la donnée
        let cached = fixture
            .cache_counter_repo()
            .get_counters(profile_id)
            .await?;
        assert_eq!(cached.followers_count().value(), 1250);
        Ok(())
    }

    // --- TESTS DE LA BARRIÈRE D'IDEMPOTENCE ---

    #[tokio::test]
    async fn test_ensure_executable_should_fail_on_region_mismatch() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let command_id = Uuid::now_v7();

        let context_region_code = fixture.region().inner();
        let wrong_region_code = match context_region_code {
            RegionCode::EU => RegionCode::US,
            _ => RegionCode::EU,
        };

        // On crée le Value Object Region à partir de l'enum brute (Infaillible, pas de try_new)
        let wrong_region = Region::from_raw(wrong_region_code);

        // When
        // Hypothèse : Ta fonction 'ensure_executable' prend maintenant un '&Region' ou un '&RegionCode'
        let result = fixture
            .social_ctx()
            .ensure_executable(command_id, &wrong_region)
            .await;

        // Then
        assert!(
            result.is_err(),
            "L'exécution aurait dû être bloquée pour cause de mismatch de région"
        );
        let error = result.unwrap_err();

        assert_eq!(error.code, ErrorCode::ValidationFailed);
        assert!(
            error.message.contains("region"),
            "Le message aurait dû cibler le champ 'region', reçu: '{}'",
            error.message
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_executable_should_return_false_if_command_already_exists() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let command_id = Uuid::now_v7();
        let region = fixture.region();

        // On simule une commande déjà exécutée enregistrée dans le stub d'idempotence
        fixture.idempotency_repo().save(None, &command_id).await?;

        // When
        let executable = fixture
            .social_ctx()
            .ensure_executable(command_id, &region)
            .await?;

        // Then
        assert!(
            !executable,
            "La commande rejouée devrait être bloquée (executable = false)"
        );
        Ok(())
    }

    // --- TESTS DES ÉCRITURES ET MUTATIONS (SANS TRANSACTION DISTRIBUÉE) ---

    #[tokio::test]
    async fn test_save_relation_should_execute_gpc_flow_synchronously() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let command_id = Uuid::now_v7();

        let follower_id = fixture.target_profile_id();
        let following_id = ProfileId::generate(fixture.region());

        let mut relation = FollowRelation::builder(follower_id, following_id).build()?;

        // When
        fixture
            .social_ctx()
            .save_relation(&mut relation, command_id)
            .await?;

        // Then
        // 1. Vérification de la barrière d'idempotence posée dans Redis
        assert!(fixture.idempotency_repo().exists(None, &command_id).await?);

        // 2. Vérification de l'écriture Graphe synchrone (ScyllaDB)
        fixture
            .assert_relation_exists(follower_id, following_id)
            .await;

        // 3. Vérification du Hot Path Compteurs & Marquage Dirty dans Redis
        fixture.assert_counters_values(follower_id, 0, 1).await; // Il suit quelqu'un (+1 following)
        fixture.assert_counters_values(following_id, 1, 0).await; // L'autre gagne un follower (+1 follower)
        Ok(())
    }

    #[tokio::test]
    async fn test_save_relation_should_raise_error_on_actor_sharding_violation() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let command_id = Uuid::now_v7();

        let local_region_code = fixture.region().inner();
        let foreign_region_code = match local_region_code {
            RegionCode::EU => RegionCode::US,
            _ => RegionCode::EU,
        };

        // On utilise la méthode de génération de ton Kernel (qui prend probablement l'enum ou le VO)
        let foreign_follower = ProfileId::generate(Region::from_raw(foreign_region_code));
        let local_following = fixture.target_profile_id();

        let mut invalid_relation =
            FollowRelation::builder(foreign_follower, local_following).build()?;

        // When
        let result = fixture
            .social_ctx()
            .save_relation(&mut invalid_relation, command_id)
            .await;

        // Then
        assert!(
            result.is_err(),
            "Le sharding cross-region aurait dû bloquer l'écriture synchrone"
        );
        let error = result.unwrap_err();

        assert_eq!(error.code, ErrorCode::ValidationFailed);
        assert!(
            error.message.contains("region"),
            "L'erreur de validation aurait dû spécifier le champ 'region', reçu: '{}'",
            error.message
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_relation_should_execute_unfollow_flow_synchronously() -> Result<()> {
        // Given
        let fixture = SocialTestFixture::new();
        let command_id = Uuid::now_v7();

        let follower_id = fixture.target_profile_id();
        let following_id = ProfileId::generate(fixture.region());

        // État initial (Given): La relation existe et les compteurs sont déjà chauds
        fixture.given_existing_relation(follower_id, following_id);
        fixture.given_initial_counters(follower_id, 0, 1);
        fixture.given_initial_counters(following_id, 1, 0);

        let mut relation = FollowRelation::builder(follower_id, following_id).build()?;

        // When
        fixture
            .social_ctx()
            .delete_relation(&mut relation, command_id)
            .await?;

        // Then
        // 1. Idempotence verrouillée
        assert!(fixture.idempotency_repo().exists(None, &command_id).await?);

        // 2. Retrait immédiat du graphe (ScyllaDB)
        fixture
            .assert_relation_does_not_exist(follower_id, following_id)
            .await;

        // 3. Décrémentation atomique Redis & marquage Dirty
        fixture.assert_counters_values(follower_id, 0, 0).await;
        fixture.assert_counters_values(following_id, 0, 0).await;
        Ok(())
    }
}
