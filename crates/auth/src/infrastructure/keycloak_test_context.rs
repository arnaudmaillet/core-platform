#![cfg(feature = "test-utils")]

use shared_kernel::core::{Error, Result};
use shared_kernel::types::SubId;
use shared_kernel::security::JwtToken;
use std::sync::Arc;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;

use crate::{KeycloakValidator, TokenValidator};

static KEYCLOAK_INSTANCE: OnceCell<KeycloakSingleton> = OnceCell::const_new();

pub struct KeycloakAuthResponse {
    pub token: JwtToken,
    pub sub_id: SubId,
}

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

    pub async fn get_admin_token(&self) -> Result<KeycloakAuthResponse> {
        let client = reqwest::Client::new();
        let token_url = format!("{}/realms/master/protocol/openid-connect/token", self.uri);
        let params = [
            ("client_id", "admin-cli"),
            ("username", "admin"),
            ("password", "admin"),
            ("grant_type", "password"),
        ];

        let mut last_error = String::new();

        // On garde la boucle de retry car Keycloak peut mettre quelques secondes
        // à être opérationnel APRÈS que le port soit ouvert.
        for i in 0..10 {
            let response = client.post(&token_url).form(&params).send().await;

            match response {
                Ok(res) if res.status().is_success() => {
                    let json: serde_json::Value = res.json().await.map_err(|_| {
                        Error::internal("Invalid JSON response from Keycloak".to_string())
                    })?;

                    let raw_token = json["access_token"]
                        .as_str()
                        .ok_or_else(|| Error::internal("access_token missing".to_string()))?;

                    let jwt_token = JwtToken::try_new(raw_token)?;

                    // Extraction du SubId avec validation réelle (pas de hardcode)
                    let sub_id_str = match json["sub"].as_str() {
                        Some(s) => s.to_string(),
                        None => {
                            let claims = self.validator.validate(&jwt_token).map_err(|e| {
                                Error::internal(format!("JWT Validation failed: {:?}", e))
                            })?;
                            claims.sub_id.to_string()
                        }
                    };

                    return Ok(KeycloakAuthResponse {
                        token: jwt_token,
                        sub_id: SubId::try_new(sub_id_str)?,
                    });
                }
                Ok(res) => {
                    last_error = format!(
                        "Status: {}, Body: {}",
                        res.status(),
                        res.text().await.unwrap_or_default()
                    );
                }
                Err(e) => {
                    last_error = format!("Connection error: {}", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Err(Error::internal(format!(
            "Keycloak admin auth failed after multiple retries. Last error: {}",
            last_error
        )))
    }
}
