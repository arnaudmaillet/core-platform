// crates/infra-test/src/keycloak/keycloak_test_context.rs

use shared_kernel::core::{Error, Result};
use shared_kernel::security::JwtToken;
use shared_kernel::types::SubId;
use std::sync::Arc;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;

use auth::{KeycloakValidator, TokenValidator};

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
    pub validator: Arc<dyn TokenValidator>,
    pub uri: String,
    pub realm: String,
    pub audience: String,
}

impl KeycloakTestContext {
    pub async fn restore(realm_name: &str, audience: String) -> Self {
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
        let validator = KeycloakValidator::new(&instance.uri, realm_name, audience.to_string())
            .await
            .expect("Failed to create KeycloakValidator");

        Self {
            validator: Arc::new(validator) as Arc<dyn TokenValidator>,
            uri: instance.uri.clone(),
            realm: realm_name.to_string(),
            audience,
        }
    }
}
