//! Applies the service's six `.cql` migrations against the ephemeral node.
//!
//! The real migration assets are read from disk at runtime (not `include_str!`),
//! so a malformed `.cql` fails the suite loudly — the test exercises exactly what
//! gets deployed.
//!
//! ## Single-node replication adaptation
//!
//! `0001_create_keyspace.cql` provisions `chat` with
//! `NetworkTopologyStrategy {datacenter1: 3}`. The durable writer uses a
//! LocalQuorum profile, which needs two live replicas — impossible on a
//! single-node container. We therefore rewrite *only* the keyspace's replication
//! clause to `SimpleStrategy RF=1`; tables (0002–0006) are applied verbatim. This
//! is a test-topology adaptation, not a schema change — cluster slot/replication
//! behaviour belongs to a separate multi-node CI guard.

use std::fs;
use std::sync::Arc;

use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};

/// Builds a keyspace-less session to the contact point, then applies every
/// migration in lexical order and blocks on schema agreement.
pub async fn apply_all(contact_point: &str) {
    let config = ScyllaConfig {
        contact_points: vec![contact_point.to_owned()],
        keyspace: None,
        ..ScyllaConfig::default()
    };
    let client = Arc::new(
        ScyllaSessionBuilder::new(config)
            .build()
            .await
            .expect("integration: failed to connect to ScyllaDB for migration"),
    );
    let session = client.session.get_session();

    for statement in load_statements() {
        session
            .query_unpaged(statement.as_str(), &[])
            .await
            .unwrap_or_else(|e| panic!("migration statement failed: {e}\n--- statement ---\n{statement}"));
    }

    session
        .await_schema_agreement()
        .await
        .expect("integration: schema did not converge after migrations");
}

/// Reads the `migrations/` directory, strips line comments, splits on `;`, and
/// adapts the keyspace replication for a single node.
fn load_statements() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read migrations dir '{dir}': {e}"))
        .map(|entry| entry.expect("bad dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cql"))
        .collect();
    files.sort();

    let mut statements = Vec::new();
    for path in files {
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read migration '{}': {e}", path.display()));

        // Drop `--` line comments before splitting; each file is a comment header
        // followed by one DDL statement.
        let code: String = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");

        for piece in code.split(';') {
            let trimmed = piece.trim();
            if trimmed.is_empty() {
                continue;
            }
            statements.push(adapt_keyspace(trimmed));
        }
    }
    statements
}

/// Rewrites the keyspace replication clause for a single-node container; passes
/// every other statement through unchanged.
fn adapt_keyspace(statement: &str) -> String {
    if statement.to_uppercase().contains("CREATE KEYSPACE") {
        "CREATE KEYSPACE IF NOT EXISTS chat \
         WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1} \
         AND durable_writes = true"
            .to_owned()
    } else {
        statement.to_owned()
    }
}
