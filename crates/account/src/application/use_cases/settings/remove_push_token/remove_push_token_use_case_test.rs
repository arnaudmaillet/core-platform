#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::remove_push_token::{
        RemovePushTokenCommand, RemovePushTokenUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountSettings};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{PushToken, RegionCode};
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_remove_push_token_success() {
        let f = TestFixture::new(RemovePushTokenUseCase::new);
        let account_id = f.account_id();
        let token_to_keep = PushToken::try_new("token_keep_456").unwrap();
        let token_to_remove = PushToken::try_new("token_remove_123").unwrap();

        // 1. Arrange : On prépare des settings avec DEUX tokens
        let mut settings = AccountSettings::builder(account_id).build();
        settings.add_push_token(token_to_remove.clone()).unwrap();
        settings.add_push_token(token_to_keep.clone()).unwrap();
        settings.pull_events(); // On vide les events d'ajout initiaux
        let version_after_setup = settings.version(); // Devrait être 3 (Init + Add + Add)

        f.settings_repo().insert(settings);

        let cmd = RemovePushTokenCommand {
            account_id,
            token: token_to_remove.clone(),
        };

        // 2. Act : On s'attend à recevoir les AccountSettings mis à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La suppression du token devrait réussir");
        let updated = result.unwrap();

        // Vérification de l'état mémoire
        assert!(
            !updated.push_tokens().contains(&token_to_remove),
            "Le token doit être supprimé"
        );
        assert!(
            updated.push_tokens().contains(&token_to_keep),
            "Le token à garder doit toujours être là"
        );
        assert_eq!(updated.version(), version_after_setup + 1);

        // 4. Persistence
        let saved = f
            .settings_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(!saved.push_tokens().contains(&token_to_remove));

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::PUSH_TOKEN_REMOVED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::PUSH_TOKEN_REMOVED.to_string())
        );
    }

    #[tokio::test]
    async fn test_remove_push_token_idempotency() {
        let f = TestFixture::new(RemovePushTokenUseCase::new);
        let account_id = f.account_id();
        let non_existent_token = PushToken::try_new("valid_but_missing_token").unwrap();

        // 1. Arrange : Settings avec une liste de tokens vide (Version 1)
        let settings = AccountSettings::builder(account_id).build();
        f.settings_repo().insert(settings);

        let cmd = RemovePushTokenCommand {
            account_id,
            token: non_existent_token,
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(
            returned.version(),
            1,
            "La version ne doit pas augmenter pour une suppression inexistante"
        );
        assert!(returned.push_tokens().is_empty());

        // 4. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(RemovePushTokenUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        let token = PushToken::try_new("valid_token_123").unwrap();

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

        let cmd = RemovePushTokenCommand { account_id, token };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
