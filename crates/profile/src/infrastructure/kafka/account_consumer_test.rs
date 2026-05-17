#[cfg(test)]
mod tests {
    use crate::{
        application::utils::ProfileTestFixture, kafka::AccountConsumer,
        repositories::ProfileRepository, types::Handle,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_on_message_received_success() -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus(), f.app_ctx().clone());

        let account_id = Uuid::new_v4();
        // 💡 Ajusté : Utilise le tag exact "AccountRegistered" et les bons champs
        let payload = json!({
            "type": "AccountRegistered",
            "data": {
                "account_id": account_id,
                "region": f.region().to_string(),
                "email": "jean.dupont@test.com"
            }
        });
        let raw_payload = serde_json::to_vec(&payload)?;

        // Act
        let result = consumer.on_message_received(&raw_payload).await;

        // Assert
        assert!(
            result.is_ok(),
            "Le consommateur aurait dû traiter le message avec succès"
        );

        let expected_handle_str = format!("user_{}", &account_id.to_string()[0..8]);
        let expected_handle = Handle::try_new(expected_handle_str)?;

        // On vérifie que le profil a bien été poussé en base avec le handle autogénéré
        let saved_profile = f
            .profile_repo()
            .find_by_handle(&expected_handle, &f.region(), None)
            .await?
            .expect("Le profil aurait dû être créé en base de données avec le handle autogénéré");

        assert_eq!(saved_profile.account_id().uuid(), account_id);
        Ok(())
    }

    #[tokio::test]
    async fn test_on_message_received_idempotency_already_exists()
    -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());

        let account_id = Uuid::new_v4();
        let generated_handle_str = format!("user_{}", &account_id.to_string()[0..8]);

        // On simule qu'un profil possède déjà EXACTEMENT ce handle généré en base
        let existing_profile = f.builder(&generated_handle_str).build()?;
        f.profile_repo().save_direct(existing_profile).await;

        // On rejoue le même événement (déclencheur de l'idempotence)
        let payload = json!({
            "type": "AccountRegistered",
            "data": {
                "account_id": account_id,
                "region": f.region().to_string(),
                "email": "jean.dupont@test.com"
            }
        });
        let raw_payload = serde_json::to_vec(&payload)?;

        // Act
        let result = consumer.on_message_received(&raw_payload).await;

        // Assert
        assert!(
            result.is_ok(),
            "Un doublon Kafka générant le même handle doit être acquitté sans erreur (idempotence)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_on_message_received_ignored_event() -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());
        let payload = json!({
            "type": "AccountPasswordChanged", // 💡 Aligné sur les types d'events PascalCase d'Account
            "data": {
                "account_id": Uuid::new_v4()
            }
        });
        let raw_payload = serde_json::to_vec(&payload)?;

        // Act
        let result = consumer.on_message_received(&raw_payload).await;

        // Assert
        assert!(
            result.is_ok(),
            "Les événements inconnus doivent tomber dans le #[serde(other)] et être ignorés silencieusement"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_on_message_received_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());

        let raw_payload = b"{ invalid json ...";

        // Act
        let result = consumer.on_message_received(raw_payload).await;

        // Assert
        assert!(
            result.is_err(),
            "Un payload JSON structurellement corrompu doit remonter une erreur pour ne pas perdre le message"
        );
        Ok(())
    }
}
