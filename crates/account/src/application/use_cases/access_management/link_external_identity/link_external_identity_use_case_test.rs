#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::link_external_identity::{
        LinkExternalIdentityCommand, LinkExternalIdentityUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_link_external_identity_success() {
        // 1. Setup via la Fixture
        let f = TestFixture::new(LinkExternalIdentityUseCase::new);
        let account_id = f.account_id();
        let new_ext = ExternalId::from_raw("google_123");

        // 2. Arrange : Compte existant sans lien externe (ou lien vide)
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                f.region(),
                Email::try_new("alex@test.com").unwrap(),
                ExternalId::from_raw(""),
            )
            .build(),
        );

        let cmd = LinkExternalIdentityCommand {
            account_id: account_id,
            external_id: new_ext.clone(),
        };

        // 3. Act
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // 4. Assert
        assert!(result.is_ok());
        
        // Vérification via le helper find_by_id du stub
        let saved = f.identity_repo().find_by_id(&account_id).unwrap();
        assert_eq!(saved.external_id(), &new_ext);
        assert_eq!(saved.version(), 2);
        assert_eq!(f.outbox_repo().count(), 1, "Un événement de linkage attendu");
    }

    #[tokio::test]
    async fn test_link_external_identity_conflict_already_taken() {
        let f = TestFixture::new(LinkExternalIdentityUseCase::new);
        let shared_ext = ExternalId::from_raw("google_123");

        // Arrange: Alice possède déjà l'ID externe
        f.identity_repo().insert(
            AccountIdentity::builder(
                f.account_id(),
                f.region(),
                Email::try_new("alice@test.com").unwrap(),
                shared_ext.clone(),
            )
            .build(),
        );

        // Bob (un autre ID) essaie de lier le même ID externe
        // On simule Bob en créant une commande pour un AUTRE ID que celui d'Alice
        let bob_id = shared_kernel::domain::value_objects::AccountId::new();
        let cmd = LinkExternalIdentityCommand {
            account_id: bob_id,
            external_id: shared_ext,
        };

        // Act
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::AlreadyExists { field, .. }) if field == "external_id"
        ));
    }

    #[tokio::test]
    async fn test_link_external_identity_idempotency() {
        let f = TestFixture::new(LinkExternalIdentityUseCase::new);
        let ext_id = ExternalId::from_raw("steam_456");

        // Arrange: Le compte a déjà cet ID externe
        f.identity_repo().insert(
            AccountIdentity::builder(
                f.account_id(),
                f.region(),
                Email::try_new("g@m.com").unwrap(),
                ext_id.clone(),
            )
            .build(),
        );

        let cmd = LinkExternalIdentityCommand {
            account_id: f.account_id(),
            external_id: ext_id,
        };

        // Act
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(f.outbox_repo().count(), 0, "Aucun changement ni événement si déjà lié");
        
        let saved = f.identity_repo().find_by_id(&f.account_id()).unwrap();
        assert_eq!(saved.version(), 1, "La version ne doit pas avoir augmenté");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(LinkExternalIdentityUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        let ext_id = ExternalId::from_raw("steam_456");

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

        let cmd = LinkExternalIdentityCommand {
            account_id: f.account_id(),
            external_id: ext_id,
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // ASSERT : On vérifie l'obfuscation de sécurité
        // Le compte existe en base, mais le contexte doit dire "NotFound"
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}