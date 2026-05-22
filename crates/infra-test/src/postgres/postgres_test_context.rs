// crates/infra-test/src/postgres/postgres_test_context.rs

use crate::PostgresTestContextBuilder;
use infra_sqlx::PostgresContext;
use infra_sqlx::sqlx::migrate::Migrator;
use infra_sqlx::sqlx::postgres::PgPoolOptions;
use infra_sqlx::sqlx::{Executor, PgPool};
use std::path::Path;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres as PostgresImage;

pub struct PostgresTestContext {
    context: PostgresContext,
    container: ContainerAsync<PostgresImage>,
}

impl PostgresTestContext {
    pub fn builder() -> PostgresTestContextBuilder {
        PostgresTestContextBuilder::new()
    }

    pub async fn restore(builder: PostgresTestContextBuilder) -> Self {
        // 1. Démarrage container (Identique à ton code)
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

        // 2. RÉSOLUTION DES CHEMINS (Identique à ton code, on garde juste la liste)
        let mut paths_to_run = Vec::new();
        let mut root_path_buf = std::path::PathBuf::new();

        if builder.run_kernel_migrations {
            let manifest_dir =
                std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR introuvable");
            let current_path = Path::new(&manifest_dir);

            let mut root_path = current_path;
            while !root_path.join("crates").exists() && root_path.parent().is_some() {
                root_path = root_path.parent().unwrap();
            }

            // On sauvegarde la racine pour les domaines
            root_path_buf = root_path.to_path_buf();

            let kernel_path = root_path.join("crates/shared-kernel/migrations/postgres");
            tracing::info!(resolved_kernel_path = ?kernel_path, "Résolution dynamique du chemin Kernel");

            if kernel_path.exists() {
                paths_to_run.push(kernel_path.to_string_lossy().into_owned());
            } else {
                panic!("❌ IMPOSSIBLE DE TROUVER LES MIGRATIONS KERNEL !");
            }
        } else {
            // Sécurité au cas où run_kernel_migrations serait false, on calcule quand même la racine
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let mut root_path = Path::new(&manifest_dir);
            while !root_path.join("crates").exists() && root_path.parent().is_some() {
                root_path = root_path.parent().unwrap();
            }
            root_path_buf = root_path.to_path_buf();
        }

        for p in &builder.migrations {
            let path = Path::new(p);

            // Si le chemin est absolu ou valide localement, on le garde.
            // Sinon, on le résout depuis la racine détectée du workspace.
            let final_path = if path.exists() {
                path.to_path_buf()
            } else {
                root_path_buf.join(p)
            };

            if final_path.exists() {
                let path_str = final_path.to_string_lossy().into_owned();
                paths_to_run.push(path_str);
            } else {
                panic!(
                    "❌ MIGRATION PATH NOT FOUND: '{}'. \n\
                    Vérifie la syntaxe depuis la racine du workspace.\n\
                    Chemin tenté (canonicalized): {:?}",
                    p,
                    std::fs::canonicalize(&final_path)
                );
            }
        }

        // 3. EXÉCUTION PROPRE AVEC LE MIGRATOR SQLX
        if let Some(kernel_path) = paths_to_run.first() {
            tracing::info!(migration_path = %kernel_path, "Applying Kernel migrations via SQLx Migrator");
            let migrator = Migrator::new(Path::new(kernel_path)).await.unwrap();
            migrator
                .run(&pool)
                .await
                .expect("Failed to apply Kernel migrations");
        }

        // Les domaines suivants sont exécutés comme du SQL brut pour éviter l'erreur VersionMissing
        for path in paths_to_run.iter().skip(1) {
            tracing::info!(migration_path = %path, "Applying Domain migrations via raw SQL execution");

            let mut entries = std::fs::read_dir(Path::new(path))
                .expect("Failed to read domain migrations directory");

            // On trie les fichiers par nom pour respecter l'ordre chronologique
            let mut files = Vec::new();
            while let Some(Ok(entry)) = entries.next() {
                if entry.path().extension().map_or(false, |ext| ext == "sql") {
                    files.push(entry.path());
                }
            }
            files.sort();

            for file in files {
                tracing::info!(file_path = ?file, "Executing domain migration file");
                let sql = std::fs::read_to_string(&file).expect("Failed to read migration file");

                pool.execute(sql.as_str())
                    .await
                    .map_err(|e| {
                        tracing::error!(file = ?file, error = %e, "Raw SQL migration failed");
                        e
                    })
                    .expect("Failed to apply domain migration");
            }
        }

        // 4. CONSTRUCTION DU CONTEXTE (Identique à ton code)
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
        Self { context, container }
    }

    pub fn pool(&self) -> PgPool {
        self.context.pool()
    }

    pub fn url(&self) -> &str {
        self.context.url()
    }
}
