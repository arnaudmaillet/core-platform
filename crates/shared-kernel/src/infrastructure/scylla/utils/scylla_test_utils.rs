// crates/shared-kernel/src/infrastructure/scylla/utils/scylla_test_utils.rs

#[cfg(feature = "test-utils")]

use std::sync::Arc;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use tokio::sync::{Mutex, OnceCell};

struct ScyllaSingleton {
    // On stocke la session Arc pour la partager
    session: Arc<Session>,
    _container: ContainerAsync<GenericImage>,
}

static SCYLLA_INSTANCE: OnceCell<ScyllaSingleton> = OnceCell::const_new();
static MIGRATIONS_DONE: OnceCell<()> = OnceCell::const_new();

pub async fn setup_test_scylla(module_migration_paths: &[&str]) -> Arc<Session> {
    let instance = SCYLLA_INSTANCE.get_or_init(|| async {
        // --- 1. Démarrage Container ---
        let port = ContainerPort::Tcp(9042);
        let node = GenericImage::new("scylladb/scylla", "6.2.1")
            .with_exposed_port(port)
            .with_wait_for(WaitFor::message_on_either_std("init - serving"))
            .with_cmd(["--developer-mode", "1", "--smp", "1", "--memory", "1G"])
            .start()
            .await
            .expect("Scylla failed to start");

        let host_port = node.get_host_port_ipv4(port).await.unwrap();
        let uri = format!("127.0.0.1:{}", host_port);

        // --- 2. Création Session ---
        let session = SessionBuilder::new()
            .known_node(&uri)
            .disallow_shard_aware_port(true)
            .build()
            .await
            .expect("Failed to create Scylla session");

        let shared_session = Arc::new(session);

        // 🚨 ON CRÉE LE KEYSPACE UNIQUE ICI (UNE SEULE FOIS)
        let ks_name = "integration_tests";
        shared_session.query_unpaged(format!(
            "CREATE KEYSPACE IF NOT EXISTS {} WITH REPLICATION = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
            ks_name
        ), ()).await.unwrap();

        shared_session.use_keyspace(ks_name, false).await.unwrap();

        // Table de migration unique
        shared_session.query_unpaged(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version bigint PRIMARY KEY, description text, applied_at timestamp)",
            ()
        ).await.unwrap();

        ScyllaSingleton { session: shared_session, _container: node }
    }).await;

    let session = instance.session.clone();

    MIGRATIONS_DONE.get_or_init(|| async {
        println!("🚀 Running Scylla migrations (once)...");

        // On s'assure que le keyspace est prêt
        let ks_name = "integration_tests";
        session.query_unpaged(format!(
            "CREATE KEYSPACE IF NOT EXISTS {} WITH REPLICATION = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
            ks_name
        ), ()).await.unwrap();

        session.use_keyspace(ks_name, false).await.unwrap();

        session.query_unpaged(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version bigint PRIMARY KEY, description text, applied_at timestamp)",
            ()
        ).await.unwrap();

        run_scylla_migrations_internal(&session, module_migration_paths).await;
        ()
    }).await;

    session
}


async fn run_scylla_migrations_internal(session: &Arc<Session>, paths: &[&str]) {
    let mut all_paths = Vec::new();

    // 1. Chemins par défaut du Kernel (Scylla)
    let possible_kernel_paths = [
        "../shared-kernel/migrations/scylla",
        "./crates/shared-kernel/migrations/scylla",
    ];
    if let Some(kp) = possible_kernel_paths.iter().find(|p| std::path::Path::new(p).exists()) {
        all_paths.push(kp.to_string());
    }

    // 2. Chemins spécifiques au module
    for p in paths {
        if std::path::Path::new(p).exists() {
            all_paths.push(p.to_string());
        }
    }

    // 3. Traitement de chaque dossier
    for path in all_paths {
        let mut entries = std::fs::read_dir(path).expect("Invalid Scylla migration path");
        let mut migrations = Vec::new();

        while let Some(Ok(entry)) = entries.next() {
            let file_name = entry.file_name().into_string().unwrap();
            if file_name.ends_with(".cql") {
                // Extraction de la version (format attendu : 202601010000_init.cql)
                let version: i64 = file_name
                    .split('_')
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("Scylla migration file must start with a version number followed by _");

                let content = std::fs::read_to_string(entry.path()).unwrap();
                migrations.push((version, file_name, content));
            }
        }

        // Tri par version pour respecter l'ordre chronologique
        migrations.sort_by_key(|m| m.0);

        // 4. Exécution unitaire
        for (version, file_name, content) in migrations {
            let check_query = "SELECT version FROM schema_migrations WHERE version = ?";
            let result = session.query_unpaged(check_query, (version,)).await.unwrap();

            let already_applied = result
                .into_rows_result()
                .map(|r| r.rows_num() > 0)
                .unwrap_or(false);

            if !already_applied {
                for statement in content.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    session.query_unpaged(statement, ()).await
                        .expect(&format!("Failed to execute statement in {}", file_name));
                }

                let log_query = "INSERT INTO schema_migrations (version, description, applied_at) VALUES (?, ?, toTimestamp(now()))";
                session.query_unpaged(log_query, (version, file_name))
                    .await
                    .expect("Failed to log Scylla migration");
            }
        }
    }
}