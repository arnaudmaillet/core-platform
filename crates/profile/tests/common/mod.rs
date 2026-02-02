// crates/profile/tests/common/mod.rs

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::Path;
use sqlx::migrate::Migrator;
use sqlx::Executor;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use testcontainers::ContainerAsync; // Import nécessaire pour l'annotation de type

pub async fn setup_test_db() -> (PgPool, ContainerAsync<PostgresImage>) {
    // 1. Configurer les paramètres Postgres D'ABORD
    // 2. Changer l'image et le tag EN DERNIER
    let container = PostgresImage::default()
        .with_user("test")
        .with_password("test")
        .with_db_name("test_db")
        .with_name("postgis/postgis")
        .with_tag("16-3.4-alpine")
        .start()
        .await
        .expect("Échec du démarrage de PostGIS");

    let host_port = container.get_host_port_ipv4(5432).await.expect("Failed to get port");
    let conn_str = format!("postgres://test:test@127.0.0.1:{}/test_db", host_port);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&conn_str)
        .await
        .expect("Échec de connexion à la DB de test");

    // 1. Injection de la fonction (Brute)
    pool.execute(r#"
        CREATE OR REPLACE FUNCTION public.trigger_set_timestamp()
        RETURNS TRIGGER AS $$
        BEGIN
            NEW.updated_at = NOW();
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
    "#).await.unwrap();

    // 2. Chargement des migrateurs
    let m_kernel = Migrator::new(Path::new("../shared-kernel/migrations/postgres")).await.unwrap();
    let m_profile = Migrator::new(Path::new("./migrations/postgres")).await.unwrap();

    // 3. Exécution brute de chaque fichier de migration
    // On utilise pool.execute(&m.sql) car .execute() du trait Executor
    // supporte les commandes multiples séparées par des ;
    for m in m_kernel.migrations.iter() {
        pool.execute(&*m.sql).await
            .map_err(|e| format!("Erreur kernel {}: {}", m.description, e)).unwrap();
    }

    for m in m_profile.migrations.iter() {
        pool.execute(&*m.sql).await
            .map_err(|e| format!("Erreur profile {}: {}", m.description, e)).unwrap();
    }

    (pool, container)
}