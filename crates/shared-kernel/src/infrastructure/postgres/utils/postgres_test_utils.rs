// crates/shared-kernel/src/infrastructure/postgres/utils/test_utils.rs
#![cfg(feature = "test-utils")]

use std::path::Path;
use sqlx::{Executor, PgPool};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;

pub async fn setup_test_postgres(
    module_migrations: &[&str]
) -> (PgPool, ContainerAsync<PostgresImage>) {
    // 1. Démarrage container (Inchangé)
    let container = PostgresImage::default()
        .with_user("test")
        .with_password("test")
        .with_db_name("test_db")
        .with_name("postgis/postgis")
        .with_tag("16-3.4-alpine")
        .start()
        .await
        .expect("Échec PostGIS");

    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!("postgres://test:test@127.0.0.1:{}/test_db", host_port);
    let pool = PgPoolOptions::new().connect(&conn_str).await.unwrap();

    // 2. Initialisation système ET table de migration SQLx
    pool.execute(r#"
        -- La fonction de timestamp
        CREATE OR REPLACE FUNCTION public.trigger_set_timestamp()
        RETURNS TRIGGER AS $$
        BEGIN
            NEW.updated_at = NOW();
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;

        -- LA TABLE CRUCIALE POUR SQLX
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            success BOOLEAN NOT NULL,
            checksum BYTEA NOT NULL,
            execution_time BIGINT NOT NULL
        );
    "#).await.expect("Failed to initialize system tables");

    // 3. RÉSOLUTION DES CHEMINS
    let mut paths_to_run = Vec::new();

    // Résolution Kernel
    let possible_kernel_paths = [
        "crates/shared-kernel/migrations/postgres",
        "../shared-kernel/migrations/postgres",
    ];
    if let Some(kp) = possible_kernel_paths.iter().find(|p| Path::new(p).exists()) {
        paths_to_run.push(kp.to_string());
    }

    // Résolution Module (On transforme les chemins relatifs en chemins Bazel si nécessaire)
    for p in module_migrations {
        if Path::new(p).exists() {
            paths_to_run.push(p.to_string());
        } else {
            // HACK BAZEL: Si on ne trouve pas "./migrations/postgres",
            // on cherche "crates/profile/migrations/postgres"
            let bazel_path = format!("crates/profile/{}", p.trim_start_matches("./"));
            if Path::new(&bazel_path).exists() {
                println!("✅ Bazel Auto-fix: Found Module migrations at: {}", bazel_path);
                paths_to_run.push(bazel_path);
            } else {
                println!("⚠️ WARNING: Migration path not found: {} (tried {})", p, bazel_path);
            }
        }
    }

    // 4. EXÉCUTION UNITAIRE (Corrigé pour matcher exactement la table SQLx)
    for path in paths_to_run {
        let migrator = Migrator::new(Path::new(&path)).await.expect("Invalid migration path");

        for migration in migrator.migrations.iter() {
            let row: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
                .bind(migration.version)
                .fetch_one(&pool)
                .await
                .unwrap_or((false,));

            if !row.0 {
                // Application du SQL
                pool.execute(&*migration.sql).await.expect("Failed to apply migration");

                // Log avec tous les champs requis par SQLx
                sqlx::query(
                    "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
                     VALUES ($1, $2, TRUE, $3, 0)"
                )
                    .bind(migration.version)
                    .bind(&*migration.description)
                    .bind(&*migration.checksum)
                    .execute(&pool)
                    .await
                    .expect("Failed to log migration");
            }
        }
    }

    (pool, container)
}