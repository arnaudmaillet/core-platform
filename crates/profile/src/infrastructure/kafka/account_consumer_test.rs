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
        // On récupère l'app_ctx et le bus mocké/stubbé depuis notre fixture de test standard
        let consumer = AccountConsumer::new(f.bus(), f.app_ctx().clone());

        let account_id = Uuid::new_v4();
        let payload = json!({
            "type": "account.created",
            "data": {
                "account_id": account_id,
                "region": f.region().to_string(),
                "username": "jean_dupont"
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

        // On vérifie que le profil a bien été poussé en base par le bus suite à l'événement
        let saved_profile = f
            .profile_repo()
            .find_by_handle(&Handle::try_new("jean_dupont")?, &f.region(), None)
            .await?
            .expect("Le profil aurait dû être créé en base de données");

        assert_eq!(saved_profile.account_id().uuid(), account_id);
        Ok(())
    }

    #[tokio::test]
    async fn test_on_message_received_idempotency_already_exists()
    -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());

        // On simule qu'un profil possède déjà ce handle en base
        let existing_profile = f.builder("jean_dupont").build()?;
        f.profile_repo().save_direct(existing_profile).await;

        let payload = json!({
            "type": "account.created",
            "data": {
                "account_id": Uuid::new_v4(),
                "region": f.region().to_string(),
                "username": "jean_dupont" // Conflit de handle !
            }
        });
        let raw_payload = serde_json::to_vec(&payload)?;

        // Act
        let result = consumer.on_message_received(&raw_payload).await;

        // Assert
        // Doit renvoyer Ok(()) et ne pas crasher car l'erreur AlreadyExists est attrapée et ignorée de manière idempotente
        assert!(
            result.is_ok(),
            "Un doublon Kafka doit être acquitté sans erreur de manière idempotente"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_on_message_received_ignored_event() -> Result<(), Box<dyn std::error::Error>> {
        // Arrange
        let f = ProfileTestFixture::new();
        let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());

        // Un événement que le domaine profile ignore complètement
        let payload = json!({
            "type": "account.password_changed",
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
            "Les événements inconnus doivent être ignorés silencieusement"
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
        // Ici on s'attend à une erreur de parsing (Err) car le payload est structurellement corrompu
        assert!(
            result.is_err(),
            "Un payload JSON corrompu doit remonter une erreur"
        );
        Ok(())
    }
}
