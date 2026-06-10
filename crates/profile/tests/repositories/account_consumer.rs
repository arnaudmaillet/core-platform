// crates/profile/src/infrastructure/kafka/account_consumer_tests.rs

use profile::{kafka::AccountConsumer, repositories::ProfileRepository, types::Handle};
use profile_test_utils::ProfileTestFixture;
use serde_json::json;
use shared_kernel::types::{AccountId, Region};
use uuid::Uuid;

#[tokio::test]
async fn test_on_message_received_success() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let f = ProfileTestFixture::new();
    let consumer = AccountConsumer::new(f.bus(), f.app_ctx().clone());

    let domain_account_id = AccountId::generate();
    let raw_uuid = domain_account_id.uuid();

    let payload = json!({
        "type": "AccountRegistered",
        "data": {
            "account_id": domain_account_id.to_string(),
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

    // Le handle autogénéré doit se baser sur le même UUID
    let expected_handle_str = format!("user_{}", &raw_uuid.to_string()[0..8]);
    let expected_handle = Handle::try_new(expected_handle_str)?;

    // Signature nettoyée : plus de Region ni de _tx (None) passés au repository
    let saved_profile = f
        .profile_repo()
        .find_by_handle(&expected_handle, f.region().clone())
        .await?
        .expect("Le profil aurait dû être créé en base de données avec le handle autogénéré");

    assert_eq!(saved_profile.account_id().uuid(), raw_uuid);
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
    let existing_profile = f.builder(&generated_handle_str)?.build()?;

    // Signature nettoyée : save_direct n'attend plus la région
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
async fn test_on_message_received_ignored_cross_region_event()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let f = ProfileTestFixture::new(); // Initialisé avec f.region() par défaut (ex: EU)
    let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());

    // On simule un événement qui arrive sur le topic mais appartient à la région US
    let cross_region = if f.region() == Region::default() {
        Region::try_from("US")?
    } else {
        Region::default()
    };

    let payload = json!({
        "type": "AccountRegistered",
        "data": {
            "account_id": Uuid::new_v4().to_string(),
            "region": cross_region.to_string(),
            "email": "foreign.user@test.com"
        }
    });
    let raw_payload = serde_json::to_vec(&payload)?;

    // Act
    let result = consumer.on_message_received(&raw_payload).await;

    // Assert
    assert!(
        result.is_ok(),
        "L'événement d'une autre région doit être ignoré avec succès (No-Op) sans lever d'erreur"
    );

    // On s'assure qu'aucun profil n'a été créé dans notre silo local
    assert_eq!(f.profile_repo().count(), 0);
    Ok(())
}

#[tokio::test]
async fn test_on_message_received_ignored_event() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let f = ProfileTestFixture::new();
    let consumer = AccountConsumer::new(f.bus().clone(), f.app_ctx().clone());
    let payload = json!({
        "type": "AccountPasswordChanged",
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
