// backend/services/identity/tests/integration.rs

use identity_service::proto::identity::v1::*;
use tonic::transport::Channel;

mod embed_server {
    // On ré-exporte le main du serveur pour pouvoir le lancer dans un task
    pub use identity_service::main;
}

#[tokio::test]
async fn test_get_user_integration() {
    // Démarrer le serveur dans une tâche séparée
    let server_handle = tokio::spawn(async {
        embed_server::main().await.unwrap();
    });

    // Attendre un peu que le serveur soit prêt (port 50051)
    // En prod, on pourrait utiliser un retry ou un health check, mais ici simple sleep
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Créer un client tonic
    let channel = Channel::from_static("http://[::1]:50051")
        .connect()
        .await
        .expect("Failed to connect to server");

    let mut client = user_service_client::UserServiceClient::new(channel);

    // Appel réel
    let request = tonic::Request::new(GetUserRequest {
        user_id: "4242".to_string(),
    });

    let response = client.get_user(request).await.unwrap().into_inner();

    // Assertions
    assert_eq!(response.username, "User_4242");
    assert_eq!(response.bio, "Ingénieur Core-Platform");
    assert!(response.avatar_url.contains("dicebear"));

    // Nettoyage : arrêter le serveur
    server_handle.abort();
}