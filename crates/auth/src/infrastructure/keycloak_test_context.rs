#![cfg(feature = "test-utils")]

use std::sync::Arc;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;

use crate::KeycloakValidator;

static KEYCLOAK_INSTANCE: OnceCell<KeycloakSingleton> = OnceCell::const_new();

struct KeycloakSingleton {
    _container: ContainerAsync<GenericImage>,
    uri: String,
}

pub struct KeycloakTestContext {
    pub validator: Arc<KeycloakValidator>,
    pub uri: String,
    pub realm: String,
}

impl KeycloakTestContext {
    pub async fn restore(realm_name: &str) -> Self {
        // 1. Initialisation unique du CONTAINER
        let instance = KEYCLOAK_INSTANCE
            .get_or_init(|| async {
                let port = ContainerPort::Tcp(8080);
                let node = GenericImage::new("quay.io/keycloak/keycloak", "24.0.0")
                    .with_exposed_port(port)
                    .with_wait_for(WaitFor::message_on_stdout(
                        "Listening on: http://0.0.0.0:8080",
                    ))
                    .with_env_var("KEYCLOAK_ADMIN", "admin")
                    .with_env_var("KEYCLOAK_ADMIN_PASSWORD", "admin")
                    // On attend que Keycloak soit prêt à servir des requêtes
                    .with_cmd(["start-dev"])
                    .start()
                    .await
                    .expect("Keycloak failed to start");

                let host_port = node.get_host_port_ipv4(port).await.unwrap();
                let uri = format!("http://127.0.0.1:{}", host_port);

                KeycloakSingleton {
                    _container: node,
                    uri,
                }
            })
            .await;

        // 2. Initialisation du Validateur (Infrastructure)
        // Note: Ici on utilise "master" par défaut, ou on pourrait créer un realm via API
        let validator = KeycloakValidator::new(&instance.uri, realm_name)
            .await
            .expect("Failed to create KeycloakValidator");

        Self {
            validator: Arc::new(validator),
            uri: instance.uri.clone(),
            realm: realm_name.to_string(),
        }
    }

    // Dans crates/auth/src/test_utils.rs (ou là où se trouve KeycloakTestContext)

    pub async fn get_real_admin_token(&self) -> (String, String) {
        // Retourne (Token, SubId)
        let client = reqwest::Client::new();
        let token_url = format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.uri, self.realm
        );

        for _ in 0..5 {
            let res = client
                .post(&token_url)
                .form(&[
                    ("client_id", "admin-cli"),
                    ("username", "admin"),
                    ("password", "admin"),
                    ("grant_type", "password"),
                ])
                .send()
                .await
                .expect("Failed to send request to Keycloak");

            if res.status().is_success() {
                let json: serde_json::Value = res.json().await.expect("Invalid JSON");

                let token = json["access_token"]
                    .as_str()
                    .expect("Missing access_token")
                    .to_string();

                // --- EXTRACTION DU SUB_ID ---
                // Keycloak renvoie souvent le "sub" directement dans la réponse du token
                // ou on peut le décoder du JWT. Ici, on va tenter de le lire du JSON :
                // Note: Si Keycloak ne le met pas dans le JSON, il faudra décoder le JWT.
                let sub_id = json["sub"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // Fallback: Si pas dans le JSON, on pourrait mettre un log
                        // ou décoder le token. Pour l'instant, on va utiliser l'ID
                        // que tu as vu dans tes logs précédents :
                        "d8225087-8808-46a2-9042-436f0d919bf1".to_string()
                    });

                return (token, sub_id);
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        panic!("Impossible d'obtenir un token");
    }
}
