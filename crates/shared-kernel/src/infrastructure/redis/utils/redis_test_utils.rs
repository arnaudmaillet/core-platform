// crates/shared-kernel/src/infrastructure/redis/utils/test_utils.rs

use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis as RedisImage;

pub async fn setup_test_redis() -> (String, ContainerAsync<RedisImage>) {
    // Démarrage du container Redis standard
    let container = RedisImage::default()
        // On peut spécifier une version spécifique si besoin
        // .with_tag("7.2-alpine")
        .start()
        .await
        .expect("Échec du démarrage de Redis");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://{}:{}", host, port);

    (url, container)
}