// crates/shared-kernel/src/test_utils/scylla/scylla_test_context.rs

use crate::ScyllaTestContextBuilder;
use infra_scylla::ScyllaContext;
use infra_scylla::scylla::client::session::Session;
use infra_scylla::scylla::client::session_builder::SessionBuilder;
use once_cell::sync::Lazy;
use std::sync::Arc;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

static SCYLLA_INSTANCE: OnceCell<ScyllaSingleton> = OnceCell::const_new();
static SCYLLA_SCHEMA_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub struct ScyllaTestContext {
    context: ScyllaContext,
    keyspace: String,
}

struct ScyllaSingleton {
    _container: ContainerAsync<GenericImage>,
    uri: String,
}

impl ScyllaTestContext {
    pub fn builder() -> ScyllaTestContextBuilder {
        ScyllaTestContextBuilder::new()
    }

    pub(crate) async fn restore(builder: ScyllaTestContextBuilder) -> Self {
        // 1. Initialisation unique du CONTAINER uniquement
        let container_info = SCYLLA_INSTANCE
            .get_or_init(|| async {
                let port = ContainerPort::Tcp(9042);
                let node = GenericImage::new(&builder.image_name, &builder.image_tag)
                    .with_exposed_port(port)
                    .with_wait_for(WaitFor::message_on_either_std("init - serving"))
                    .with_cmd([
                        "--developer-mode",
                        "1",
                        "--smp",
                        "1", // Force 1 seul CPU virtuel (évite le calcul de Shards lourd)
                        "--memory",
                        "1G", // Limite la RAM pour éviter le swap agressif dans Docker
                        "--overprovisioned",
                        "1", // Indique à Scylla qu'il partage ses ressources
                    ])
                    .start()
                    .await
                    .expect("Scylla failed to start");

                let host_port = node.get_host_port_ipv4(port).await.unwrap();
                let uri = format!("127.0.0.1:{}", host_port);

                ScyllaSingleton {
                    _container: node,
                    uri,
                }
            })
            .await;

        // 2. Préparation du nom unique
        let full_uuid = Uuid::new_v4().to_string().replace("-", "");
        let ks_name = format!("{}_{}", builder.keyspace, &full_uuid[..20]);

        // 3. ⚠️ CRUCIAL : Création du Keyspace AVANT le ScyllaContext
        {
            let _guard = SCYLLA_SCHEMA_LOCK.lock().await;

            // On crée une session "brute" sans keyspace pour l'admin
            let admin_session = SessionBuilder::new()
                .known_node(&container_info.uri)
                .disallow_shard_aware_port(true)
                .build()
                .await
                .expect("Failed to create admin session for keyspace creation");

            admin_session.query_unpaged(format!(
                "CREATE KEYSPACE IF NOT EXISTS {} WITH REPLICATION = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
                ks_name
            ), ()).await.expect("Failed to create keyspace");
        }

        // 4. Maintenant que le Keyspace EXISTE en base, on peut créer le contexte de prod
        let mut scylla_builder = ScyllaContext::builder_raw()
            .with_nodes(vec![container_info.uri.clone()])
            .with_keyspace(&ks_name);

        if let Some(cfg) = builder.config {
            scylla_builder = scylla_builder.with_timeout(cfg.connect_timeout);
        }

        let context = scylla_builder
            .build()
            .await
            .expect("Failed to build ScyllaContext");

        // 5. Migrations (Maintenant la session du contexte de prod est valide)
        {
            let _guard = SCYLLA_SCHEMA_LOCK.lock().await;
            let session = context.session();

            session.query_unpaged(
                "CREATE TABLE IF NOT EXISTS schema_migrations (version bigint PRIMARY KEY, description text, applied_at timestamp)",
                ()
            ).await.expect("Failed to create migration table");

            Self::run_migrations(&session, &builder.migrations, builder.run_kernel_migrations)
                .await;
        }

        Self {
            context,
            keyspace: ks_name,
        }
    }

    async fn run_migrations(session: &Arc<Session>, paths: &[String], run_kernel: bool) {
        let mut all_paths = Vec::new();

        // 1. Chemins Kernel (Scylla) - Soumis au flag run_kernel
        if run_kernel {
            let possible_kernel_paths = [
                "crates/shared-kernel/migrations/scylla",
                "../shared-kernel/migrations/scylla",
                "./crates/shared-kernel/migrations/scylla",
            ];
            if let Some(kp) = possible_kernel_paths
                .iter()
                .find(|p| std::path::Path::new(p).exists())
            {
                println!("✅ Scylla: Found Kernel migrations at: {}", kp);
                all_paths.push(kp.to_string());
            }
        }

        // 2. Chemins Module - Utilise les String du builder
        for p in paths {
            let path = std::path::Path::new(p);
            if path.exists() {
                println!("✅ Scylla: Found migrations at: {:?}", path);
                all_paths.push(p.to_string());
            } else {
                // Ajoute un log d'erreur plus explicite pour débugger
                println!("❌ Scylla ERROR: Migration path does NOT exist: {:?}", path);
            }
        }

        for path in all_paths {
            let mut entries = std::fs::read_dir(path).expect("Invalid Scylla migration path");
            let mut migrations = Vec::new();

            while let Some(Ok(entry)) = entries.next() {
                let file_name = entry.file_name().into_string().unwrap();
                if file_name.ends_with(".cql") {
                    let version: i64 = file_name
                        .split('_')
                        .next()
                        .and_then(|v| v.parse().ok())
                        .expect("Scylla migration file must start with a version number");

                    let content = std::fs::read_to_string(entry.path()).unwrap();
                    migrations.push((version, file_name, content));
                }
            }

            migrations.sort_by_key(|m| m.0);

            for (version, file_name, content) in migrations {
                let check_query = "SELECT version FROM schema_migrations WHERE version = ?";
                let result = session
                    .query_unpaged(check_query, (version,))
                    .await
                    .unwrap();

                let already_applied = result
                    .into_rows_result()
                    .map(|r| r.rows_num() > 0)
                    .unwrap_or(false);

                if !already_applied {
                    // Split par point-virgule pour exécuter les statements CQL un par un
                    for statement in content
                        .split(';')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                    {
                        session
                            .query_unpaged(statement, ())
                            .await
                            .expect(&format!("Failed to execute statement in {}", file_name));
                    }

                    let log_query = "INSERT INTO schema_migrations (version, description, applied_at) VALUES (?, ?, toTimestamp(now()))";
                    session
                        .query_unpaged(log_query, (version, file_name))
                        .await
                        .expect("Failed to log Scylla migration");
                }
            }
        }
    }

    pub fn session(&self) -> Arc<Session> {
        self.context.session()
    }

    pub fn keyspace(&self) -> &str {
        &self.keyspace
    }

    pub fn uri(&self) -> String {
        self.context.nodes().first().cloned().unwrap_or_default()
    }
}
