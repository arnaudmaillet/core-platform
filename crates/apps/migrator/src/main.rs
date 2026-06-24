//! `migrator` — the fleet schema-migration runner.
//!
//! Applies each service's migrations against the right backend (ScyllaDB for the
//! `.cql` services, PostgreSQL for `account`'s `.sql`). Migrations are **embedded
//! at compile time** (`include_dir!`), so the binary is self-contained — no
//! source tree or mounted volume needed in the container. Application is
//! **tracked and idempotent**: each applied `(service, version)` is recorded, so
//! re-running skips what's already done.
//!
//! Usage:
//! ```text
//! migrator              # migrate every service
//! migrator chat         # migrate one service
//! ```
//!
//! Connection settings come from the same env vars the services use
//! (`SCYLLA_*`, `PG_*` / `DATABASE_URL`).
//!
//! NOTE: statement splitting strips `--` line comments and splits on `;`. Our
//! migration files contain no `;` inside string literals, so this is safe; revisit
//! if that ever changes.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use postgres_storage::{PgPoolBuilder, PostgresConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};

/// Which backend a service's migrations target.
enum Store {
    Scylla,
    Postgres,
}

struct ServiceMigrations {
    name: &'static str,
    store: Store,
    dir: Dir<'static>,
}

/// Every service's embedded migration tree, in dependency-free order (each
/// service's keyspace/schema is self-contained).
fn services() -> Vec<ServiceMigrations> {
    vec![
        ServiceMigrations { name: "chat",          store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/chat/migrations") },
        ServiceMigrations { name: "profile",       store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/profile/migrations") },
        ServiceMigrations { name: "social-graph",  store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/social-graph/migrations") },
        ServiceMigrations { name: "post",          store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/post/migrations") },
        ServiceMigrations { name: "engagement",    store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/engagement/migrations") },
        ServiceMigrations { name: "comment",       store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/comment/migrations") },
        ServiceMigrations { name: "geo-discovery", store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/geo-discovery/migrations") },
        ServiceMigrations { name: "notification",  store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/notification/migrations") },
        ServiceMigrations { name: "timeline",      store: Store::Scylla,   dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/timeline/migrations") },
        ServiceMigrations { name: "account",       store: Store::Postgres, dir: include_dir!("$CARGO_MANIFEST_DIR/../../services/account/migrations") },
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::args().nth(1);

    let all = services();
    let targets: Vec<&ServiceMigrations> = all
        .iter()
        .filter(|s| filter.as_deref().is_none_or(|f| f == s.name))
        .collect();

    if targets.is_empty() {
        anyhow::bail!(
            "unknown service '{}'; known: {}",
            filter.unwrap_or_default(),
            all.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
        );
    }

    let scylla: Vec<&ServiceMigrations> =
        targets.iter().copied().filter(|s| matches!(s.store, Store::Scylla)).collect();
    let postgres: Vec<&ServiceMigrations> =
        targets.iter().copied().filter(|s| matches!(s.store, Store::Postgres)).collect();

    if !scylla.is_empty() {
        migrate_scylla(&scylla).await.context("scylla migrations")?;
    }
    if !postgres.is_empty() {
        migrate_postgres(&postgres).await.context("postgres migrations")?;
    }

    println!("migrations complete");
    Ok(())
}

// ── ScyllaDB ─────────────────────────────────────────────────────────────────

async fn migrate_scylla(targets: &[&ServiceMigrations]) -> Result<()> {
    // No keyspace on the session: the service keyspaces are created by their own
    // 0001 migration, and every statement is keyspace-qualified.
    let mut cfg = ScyllaConfig::from_env();
    cfg.keyspace = None;
    let client = ScyllaSessionBuilder::new(cfg).build().await.context("connect")?;
    let session = client.session.get_session();

    // Tracker (SimpleStrategy RF1 — bookkeeping, not service data).
    session
        .query_unpaged(
            "CREATE KEYSPACE IF NOT EXISTS migrations WITH replication = \
             {'class': 'SimpleStrategy', 'replication_factor': 1}",
            (),
        )
        .await
        .context("create migrations keyspace")?;
    session
        .query_unpaged(
            "CREATE TABLE IF NOT EXISTS migrations.applied \
             (service text, version text, applied_at timestamp, PRIMARY KEY ((service), version))",
            (),
        )
        .await
        .context("create migrations.applied")?;

    for svc in targets {
        println!("[scylla] {}", svc.name);

        let result = session
            .query_unpaged("SELECT version FROM migrations.applied WHERE service = ?", (svc.name,))
            .await
            .context("read applied versions")?
            .into_rows_result()
            .context("applied: rows")?;
        let mut applied = BTreeSet::new();
        for row in result.rows::<(String,)>().context("applied: typed")? {
            applied.insert(row.context("applied: deser")?.0);
        }

        for (version, content) in sorted_migrations(&svc.dir) {
            if applied.contains(&version) {
                println!("  [skip] {version}");
                continue;
            }
            for stmt in split_statements(&content) {
                session
                    .query_unpaged(stmt.as_str(), ())
                    .await
                    .with_context(|| format!("{}/{version}: {}", svc.name, first_line(&stmt)))?;
            }
            session
                .query_unpaged(
                    "INSERT INTO migrations.applied (service, version, applied_at) \
                     VALUES (?, ?, toTimestamp(now()))",
                    (svc.name, version.as_str()),
                )
                .await
                .with_context(|| format!("record {}/{version}", svc.name))?;
            println!("  [done] {version}");
        }
    }

    Ok(())
}

// ── PostgreSQL ───────────────────────────────────────────────────────────────

async fn migrate_postgres(targets: &[&ServiceMigrations]) -> Result<()> {
    let pool = PgPoolBuilder::build(PostgresConfig::from_env()).await.context("connect")?;

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS schema_migrations \
         (service text NOT NULL, version text NOT NULL, \
          applied_at timestamptz NOT NULL DEFAULT now(), \
          PRIMARY KEY (service, version))",
    )
    .execute(&pool)
    .await
    .context("create schema_migrations")?;

    for svc in targets {
        println!("[postgres] {}", svc.name);

        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT version FROM schema_migrations WHERE service = $1")
                .bind(svc.name)
                .fetch_all(&pool)
                .await
                .context("read applied versions")?;
        let applied: BTreeSet<String> = rows.into_iter().map(|r| r.0).collect();

        for (version, content) in sorted_migrations(&svc.dir) {
            if applied.contains(&version) {
                println!("  [skip] {version}");
                continue;
            }
            // `raw_sql` runs the whole file (multiple statements) in one round-trip.
            sqlx::raw_sql(&content)
                .execute(&pool)
                .await
                .with_context(|| format!("{}/{version}", svc.name))?;
            sqlx::query("INSERT INTO schema_migrations (service, version) VALUES ($1, $2)")
                .bind(svc.name)
                .bind(&version)
                .execute(&pool)
                .await
                .with_context(|| format!("record {}/{version}", svc.name))?;
            println!("  [done] {version}");
        }
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Returns `(version, contents)` for each `.cql`/`.sql` file, ordered by filename
/// (which is the numeric migration prefix).
fn sorted_migrations(dir: &Dir<'static>) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = dir
        .files()
        .filter(|f| {
            matches!(f.path().extension().and_then(|e| e.to_str()), Some("cql") | Some("sql"))
        })
        .filter_map(|f| {
            let name = f.path().file_name()?.to_string_lossy().into_owned();
            let body = f.contents_utf8()?.to_owned();
            Some((name, body))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Strips `--` line comments and splits a migration file into individual
/// statements on `;`.
fn split_statements(content: &str) -> Vec<String> {
    let stripped: String = content
        .lines()
        .map(|line| match line.find("--") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    stripped
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// First non-empty line of a statement, for error context.
fn first_line(stmt: &str) -> &str {
    stmt.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(stmt)
}
