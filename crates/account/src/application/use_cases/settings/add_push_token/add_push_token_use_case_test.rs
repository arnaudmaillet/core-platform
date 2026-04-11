#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::add_push_token::{
        AddPushTokenCommand, AddPushTokenUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountSettings};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{PushToken, RegionCode};
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_success_path_full_flow() {
        let f: TestFixture<AddPushTokenUseCase> = TestFixture::new(AddPushTokenUseCase::new);
        let account_id = f.account_id();

        let settings = AccountSettings::builder(account_id).build();
        f.settings_repo().insert(settings);

        let token = PushToken::try_new("valid_push_token_long_enough").unwrap();
        let cmd = AddPushTokenCommand {
            account_id,
            token: token.clone(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());

        // Vérifier la persistance
        let saved = f
            .settings_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert!(saved.push_tokens().contains(&token));
        assert_eq!(saved.version(), 2);

        // Vérifier l'événement Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::PUSH_TOKEN_ADDED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::PUSH_TOKEN_ADDED.to_string()));
    }

    #[tokio::test]
    async fn test_idempotency_stops_execution_early() {
        let f = TestFixture::new(AddPushTokenUseCase::new);
        let account_id = f.account_id();
        let token = PushToken::try_new("idempotent_token_test_123").unwrap();

        let mut settings = AccountSettings::builder(account_id).build();
        // On simule que le token est déjà présent via l'entité directement
        settings.add_push_token(token.clone()).unwrap();
        f.settings_repo().insert(settings);

        let cmd = AddPushTokenCommand { account_id, token };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());

        // Grace au retour `false` de l'entité, le Use Case ne doit pas avoir
        // persisté d'événements dans l'outbox
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "L'idempotence aurait dû stopper le Use Case"
        );
    }

    #[tokio::test]
    async fn test_worst_case_retry_exhaustion() {
        let f = TestFixture::new(AddPushTokenUseCase::new);
        let account_id = f.account_id();


        f.settings_repo().insert(AccountSettings::builder(account_id).build());
        f.settings_repo().set_error(DomainError::ConcurrencyConflict {
            reason: "Database high pressure".into(),
        });

        let cmd = AddPushTokenCommand {
            account_id,
            token: PushToken::try_new("retry_token_test_12345").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_transaction_failure_propagation() {
        let f = TestFixture::new(AddPushTokenUseCase::new);
        let account_id = f.account_id();
        let error_msg = "Kafka/Outbox DB Error";

        f.settings_repo().insert(AccountSettings::builder(account_id).build());
        f.outbox_repo().set_error(DomainError::Internal(error_msg.into()));

        let cmd = AddPushTokenCommand {
            account_id,
            token: PushToken::try_new("token_trigger_failure_123").unwrap(),
        };

        let result = f.use_case().execute(f.ctx(), cmd).await;

        // 4. Assert
        assert!(result.is_err());
        if let Err(DomainError::Internal(msg)) = result {
            assert_eq!(msg, error_msg);
        }
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(AddPushTokenUseCase::new);
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

        let cmd = AddPushTokenCommand {
            account_id,
            token: PushToken::try_new("token_trigger_failure_123").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }  
}
