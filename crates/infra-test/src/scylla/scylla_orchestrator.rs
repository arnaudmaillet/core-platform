use anyhow::{Result, anyhow};
use infra_scylla::scylla::client::session::Session;
use infra_scylla::scylla::value::Row;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{Duration, Instant};

pub struct ScyllaTableTarget {
    pub name: String,
    pub expected_columns: usize,
}

impl ScyllaTableTarget {
    pub fn new(name: impl Into<String>, expected_columns: usize) -> Self {
        Self {
            name: name.into(),
            expected_columns,
        }
    }
}

pub struct ScyllaOrchestrator {
    session: Arc<Session>,
    migration_path: PathBuf,
    targets: Vec<ScyllaTableTarget>,
    local_region: String,
}

impl ScyllaOrchestrator {
    pub fn new(
        session: Arc<Session>,
        migration_path: impl AsRef<Path>,
        targets: Vec<ScyllaTableTarget>,
        local_region: impl Into<String>,
    ) -> Self {
        Self {
            session,
            migration_path: migration_path.as_ref().to_path_buf(),
            targets,
            local_region: local_region.into().to_lowercase(),
        }
    }

    pub async fn ensure_schema_ready(&self) -> Result<()> {
        self.ensure_dynamic_keyspace_exists().await?;

        self.apply_migrations().await?;

        self.session
            .await_schema_agreement()
            .await
            .map_err(|e| anyhow!("Schema agreement failed: {}", e))?;

        // 4. Barrière déterministe
        self.wait_for_full_conformance().await
    }

    async fn ensure_dynamic_keyspace_exists(&self) -> Result<()> {
        let dynamic_ks = format!("{}_profile_storage", self.local_region);
        let query = format!(
            "CREATE KEYSPACE IF NOT EXISTS {} WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}} AND durable_writes = true",
            dynamic_ks
        );
        self.session.query_unpaged(query, &[]).await?;
        Ok(())
    }

    async fn apply_migrations(&self) -> Result<()> {
        let dynamic_ks = format!("{}_profile_storage", self.local_region);

        self.session
            .query_unpaged(format!("USE {};", dynamic_ks), &[])
            .await?;

        let mut entries = std::fs::read_dir(&self.migration_path)?
            .filter_map(|res| res.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "cql"))
            .collect::<Vec<_>>();

        entries.sort();

        for path in entries {
            let cql_content = std::fs::read_to_string(&path)?;
            for statement in cql_content.split(';') {
                let trimmed = statement.trim();
                if !trimmed.is_empty() {
                    self.session
                        .query_unpaged(format!("{};", trimmed), &[])
                        .await
                        .map_err(|e| {
                            anyhow!("CQL Error in {:?}: {}", path.file_name().unwrap(), e)
                        })?;
                }
            }
        }
        Ok(())
    }

    async fn wait_for_full_conformance(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);

        for target in &self.targets {
            let query = format!(
                "SELECT column_name FROM system_schema.columns WHERE table_name = '{}' ALLOW FILTERING",
                target.name
            );

            let mut stable = false;
            while Instant::now() < deadline {
                if let Ok(res) = self.session.query_unpaged(query.clone(), &[]).await {
                    if res.is_rows() {
                        let rows_res = res.into_rows_result().unwrap();
                        let current_count = rows_res.rows::<Row>()?.count();

                        if current_count >= target.expected_columns {
                            stable = true;
                            break;
                        }
                    }
                }
                tokio::task::yield_now().await;
            }

            if !stable {
                return Err(anyhow!(
                    "Deterministic timeout: Table '{}' did not reach expected {} columns",
                    target.name,
                    target.expected_columns
                ));
            }
        }
        Ok(())
    }
}
