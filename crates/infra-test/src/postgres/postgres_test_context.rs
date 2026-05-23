// crates/infra-test/src/postgres/postgres_test_context.rs

use crate::PostgresTestContextBuilder;
use infra_sqlx::PostgresContext;
use infra_sqlx::sqlx::postgres::PgPoolOptions;
use infra_sqlx::sqlx::{Executor, PgPool};
use std::path::Path;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres as PostgresImage;

pub struct PostgresTestContext {
    context: PostgresContext,
    _container: ContainerAsync<PostgresImage>,
}

impl PostgresTestContext {
    pub fn builder() -> PostgresTestContextBuilder {
        PostgresTestContextBuilder::new()
    }

    pub async fn restore(builder: PostgresTestContextBuilder) -> Self {
        // 1. Démarrage container
        let container = PostgresImage::default()
            .with_user(&builder.user)
            .with_password(&builder.password)
            .with_db_name(&builder.db_name)
            .with_name(&builder.image_name)
            .with_tag(&builder.image_tag)
            .start()
            .await
            .expect("Échec démarrage Postgres");

        let host_port = container.get_host_port_ipv4(5432).await.unwrap();
        let conn_str = format!(
            "postgres://{}:{}@127.0.0.1:{}/{}",
            builder.user, builder.password, host_port, builder.db_name
        );
        let pool = PgPoolOptions::new().connect(&conn_str).await.unwrap();
        tracing::info!(db_url = %conn_str, "Postgres container started, beginning migrations");

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let mut root_path = Path::new(&manifest_dir);
        while !root_path.join("crates").exists() && root_path.parent().is_some() {
            root_path = root_path.parent().unwrap();
        }
        let root_path_buf = root_path.to_path_buf();

        // 2. EXÉCUTION DES MIGRATIONS DE FONDATION (Via le système de fichiers)
        if builder.run_kernel_migrations {
            tracing::info!("Applying Foundation migrations (Outbox/Idempotency) via filesystem");

            let foundation_path = root_path_buf.join("crates/infra-sqlx/migrations/postgres");

            if foundation_path.exists() {
                let mut entries = std::fs::read_dir(&foundation_path)
                    .expect("Failed to read foundation migrations directory");

                let mut files = Vec::new();
                while let Some(Ok(entry)) = entries.next() {
                    if entry.path().extension().map_or(false, |ext| ext == "sql") {
                        files.push(entry.path());
                    }
                }
                files.sort();

                for file in files {
                    tracing::info!(file_path = ?file, "Executing foundation migration file");
                    let sql = std::fs::read_to_string(&file)
                        .expect("Failed to read foundation migration file");

                    pool.execute(sql.as_str())
                        .await
                        .expect("Failed to apply foundation migration");
                }
            } else {
                panic!(
                    "❌ FOUNDATION MIGRATION PATH NOT FOUND: {:?}",
                    foundation_path
                );
            }
        }

        // 3. RÉSOLUTION ET EXÉCUTION DES MIGRATIONS DU MICROSERVICE (DOMAINES)
        for p in &builder.migrations {
            let path = Path::new(p);
            let final_path = if path.exists() {
                path.to_path_buf()
            } else {
                root_path_buf.join(p)
            };

            if final_path.exists() {
                tracing::info!(migration_path = ?final_path, "Applying Domain migrations via raw SQL execution");

                let mut entries = std::fs::read_dir(&final_path)
                    .expect("Failed to read domain migrations directory");

                let mut files = Vec::new();
                while let Some(Ok(entry)) = entries.next() {
                    if entry.path().extension().map_or(false, |ext| ext == "sql") {
                        files.push(entry.path());
                    }
                }
                files.sort();

                for file in files {
                    tracing::info!(file_path = ?file, "Executing domain migration file");
                    let sql =
                        std::fs::read_to_string(&file).expect("Failed to read migration file");

                    pool.execute(sql.as_str())
                        .await
                        .map_err(|e| {
                            tracing::error!(file = ?file, error = %e, "Raw SQL migration failed");
                            e
                        })
                        .expect("Failed to apply domain migration");
                }
            } else {
                panic!(
                    "❌ MIGRATION PATH NOT FOUND: '{}'. \n\
                 Vérifie la syntaxe depuis la racine du workspace.",
                    p
                );
            }
        }

        // 4. CONSTRUCTION DU CONTEXTE
        let mut context_builder = PostgresContext::builder_raw().with_url(&conn_str);
        if let Some(cfg) = builder.config {
            context_builder = context_builder
                .with_max_connections(cfg.max_connections)
                .with_min_connections(cfg.min_connections)
                .with_timeout(cfg.connect_timeout);
        }

        let context = context_builder
            .build()
            .await
            .expect("Failed to build context");

        tracing::info!("PostgresTestContext fully initialized");
        Self {
            context,
            _container: container,
        }
    }

    pub fn pool(&self) -> PgPool {
        self.context.pool()
    }

    pub fn url(&self) -> &str {
        self.context.url()
    }
}
