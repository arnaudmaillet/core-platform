#[cfg(test)]
mod tests {
    use crate::application::get_profile_by_username::{
        GetProfileByUsernameCommand, GetProfileByUsernameUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::repositories::{ProfileIdentityRepository, ProfileRepositoryStub};
    use crate::domain::value_objects::DisplayName;
    use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryStub};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use std::sync::{Arc, Mutex};

    /// Helper pour créer le Use Case avec des stubs
    fn setup(
        profile: Option<Profile>,
        cached_json: Option<String>,
        fail_cache: bool,
    ) -> GetProfileByUsernameUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        // 1. On crée le stub sur la stack (mutable)
        let mut cache_stub = CacheRepositoryStub::default();

        // 2. On le configure
        cache_stub.fail_all = fail_cache;

        if let Some(json) = cached_json {
            cache_stub
                .storage
                .lock()
                .unwrap()
                .insert("profile:un:eu:bob".to_string(), json);
        }

        // 3. On le déplace dans l'Arc seulement à la fin
        let cache = Arc::new(cache_stub);

        GetProfileByUsernameUseCase::new(repo, cache)
    }

    #[tokio::test]
    async fn test_get_profile_cache_hit_success() {
        // Arrange : Profil déjà sérialisé en cache
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        let json = serde_json::to_string(&bob).unwrap();

        // On ne met RIEN dans le repo pour prouver que le cache suffit
        let use_case = setup(None, Some(json), false);

        let cmd = GetProfileByUsernameCommand {
            username: Username::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().username().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_cache_miss_then_fill() {
        // 1. Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let username = Username::try_new("bob").unwrap();
        let cache_key = "profile:un:eu:bob";

        let bob = Profile::builder(
            account_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            username.clone(),
        )
        .build();
        let repo_stub = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(bob)),
            ..Default::default()
        });

        let cache_stub = Arc::new(CacheRepositoryStub::default());

        // On clone l'Arc pour les assertions futures
        let cache_for_assertion: Arc<CacheRepositoryStub> = Arc::clone(&cache_stub);

        let use_case = GetProfileByUsernameUseCase::new(repo_stub, cache_stub);

        let cmd = GetProfileByUsernameCommand {
            username: username.clone(),
            region,
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let result: shared_kernel::errors::AppResult<Option<String>> =
            cache_for_assertion.get(cache_key).await;
        let cached_data = result.expect("Cache stub should not fail here");

        assert!(cached_data.is_some());

        // Optionnel : vérifier que le contenu du cache est un JSON valide du profil
        let cached_json = cached_data.unwrap();
        assert!(cached_json.contains("bob"));
    }

    #[tokio::test]
    async fn test_get_profile_not_found_returns_error() {
        // Arrange : Absent partout
        let use_case = setup(None, None, false);

        let cmd = GetProfileByUsernameCommand {
            username: Username::try_new("ghost").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_profile_resilience_on_redis_failure() {
        // Arrange : Redis est en panne (fail_all = true)
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        let use_case = setup(Some(bob), None, true);

        let cmd = GetProfileByUsernameCommand {
            username: Username::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        // Le Use Case doit ignorer l'erreur Redis et renvoyer le profil de la DB
        assert!(
            result.is_ok(),
            "Le Use Case doit survivre à une panne de cache"
        );
        assert_eq!(result.unwrap().username().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_corrupted_cache_trigger_refresh() {
        // Arrange : Donnée invalide en cache
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        let use_case = setup(Some(bob), Some("{{invalid_json}}".to_string()), false);

        let cmd = GetProfileByUsernameCommand {
            username: Username::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().username().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_singleflight_deduplication() {
        // Arrange : On crée un Use Case partagé
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        // Simulation d'une latence DB pour tester le Singleflight
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(bob)),
            ..Default::default()
        });

        let cache = Arc::new(CacheRepositoryStub::default());
        let use_case = Arc::new(GetProfileByUsernameUseCase::new(repo, cache));

        let cmd = GetProfileByUsernameCommand {
            username: Username::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act : On lance 20 requêtes en même temps (Thundering Herd)
        let mut futures = vec![];
        for _ in 0..20 {
            let uc = Arc::clone(&use_case);
            let c = cmd.clone();
            futures.push(tokio::spawn(async move { uc.execute(c).await }));
        }

        // On attend tous les résultats
        let mut results = vec![];
        for f in futures {
            results.push(f.await.unwrap());
        }

        // Assert
        for r in results {
            assert!(
                r.is_ok(),
                "Toutes les requêtes concurrentes doivent réussir"
            );
        }

        // Note: Pour être parfait, on pourrait ajouter un compteur atomique dans ProfileRepoStub
        // pour vérifier que `get_full_profile_by_username` n'a été appelé qu'une SEULE fois.
    }

    #[tokio::test]
    async fn test_get_profile_singleflight_actually_deduplicates_calls() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingRepo {
            call_count: Arc<AtomicUsize>,
            profile: Profile,
        }

        #[async_trait::async_trait]
        impl crate::domain::repositories::ProfileRepository for CountingRepo {
            async fn resolve_profile_from_username(
                &self,
                _: &Username,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                let p = self.profile.clone();
                Ok(Some(p))
            }

            async fn assemble_full_profile(
                &self,
                _: &AccountId,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                Ok(Some(self.profile.clone()))
            }
            async fn fetch_identity_only(
                &self,
                _: &AccountId,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                Ok(Some(self.profile.clone()))
            }
            async fn fetch_stats_only(
                &self,
                _: &AccountId,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<crate::domain::value_objects::ProfileStats>>
            {
                Ok(None)
            }

            // Bridge vers ProfileIdentityRepository
            async fn save_identity(
                &self,
                _: &Profile,
                _: Option<&Profile>,
                _: Option<&mut dyn shared_kernel::domain::transaction::Transaction>,
            ) -> shared_kernel::errors::Result<()> {
                Ok(())
            }
            async fn exists_by_username(
                &self,
                _: &Username,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<bool> {
                Ok(true)
            }

            async fn delete_full_profile(&self, _: &AccountId, _: &RegionCode) -> shared_kernel::errors::Result<()> {
                Ok(())
            }
        }

        #[async_trait::async_trait]
        impl ProfileIdentityRepository for CountingRepo {
            async fn save(
                &self,
                _: &Profile,
                _: Option<&mut dyn shared_kernel::domain::transaction::Transaction>,
            ) -> shared_kernel::errors::Result<()> {
                Ok(())
            }
            async fn fetch(
                &self,
                _: &AccountId,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                Ok(Some(self.profile.clone()))
            }
            async fn fetch_by_username(
                &self,
                _: &Username,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                Ok(Some(self.profile.clone()))
            }
            async fn exists_by_username(
                &self,
                _: &Username,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<bool> {
                Ok(true)
            }
            async fn delete(
                &self,
                _: &AccountId,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<()> {
                Ok(())
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        let repo = Arc::new(CountingRepo {
            call_count: counter.clone(),
            profile: bob,
        });
        let cache = Arc::new(CacheRepositoryStub::default());
        let use_case = Arc::new(GetProfileByUsernameUseCase::new(repo, cache));

        let mut handles = vec![];
        for _ in 0..10 {
            let uc = Arc::clone(&use_case);
            handles.push(tokio::spawn(async move {
                uc.execute(GetProfileByUsernameCommand {
                    username: Username::try_new("bob").unwrap(),
                    region: RegionCode::from_raw("eu"),
                })
                .await
            }));
        }

        // On attend les résultats
        for h in handles {
            let res = h.await.expect("Join error");
            assert!(res.is_ok(), "Le Use Case doit réussir");
        }

        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "Le Singleflight n'a pas dédoublonné les appels !"
        );
    }
}
