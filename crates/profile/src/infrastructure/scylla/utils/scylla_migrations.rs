// crates/profile/src/infrastructure/scylla/utils/scylla_migrations.rs

use anyhow::Result;
use scylla::client::session::Session;

pub async fn run_scylla_migrations(session: &Session) -> Result<()> {
    let schema_cql = include_str!("../../../../migrations/scylla/202601030000_profile_stats.cql");

    // On découpe par ';' pour envoyer chaque commande individuellement
    for statement in schema_cql.split(';').filter(|s| !s.trim().is_empty()) {
        let query = format!("{};", statement.trim());
        session
            .query_unpaged(query, ())
            .await
            .map_err(|e| anyhow::anyhow!("Scylla migration error: {}", e))?;
    }

    Ok(())
}
