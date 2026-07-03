//! `topic-provisioner` — creates the fleet's Kafka topics ahead of the workloads.
//!
//! Brokers run with `auto.create.topics.enable=false` (MSK policy — auto-created
//! topics silently inherit broker defaults nobody chose, and MSK ships with
//! auto-creation off anyway, so an unprovisioned cluster can't be published to at
//! all). This binary derives the complete topic set from the `event-topology`
//! registry — every produced/consumed stream topic, plus the consumer runtime's
//! `<topic>.dlq` counterpart for each consumed topic — and creates them in one
//! idempotent admin call. `TOPIC_ALREADY_EXISTS` is success; anything else fails
//! the run (non-zero exit) so the deploy that depends on it does not proceed.
//!
//! Runs as an ArgoCD PreSync hook Job in each env's overlay: a new topic lands in
//! the registry and the very next sync provisions it before the fleet rolls.
//!
//! Env:
//! - `KAFKA_BROKERS` / `KAFKA_SECURITY_PROTOCOL` / `KAFKA_SASL_*` — the exact same
//!   client settings the fleet uses (`transport::kafka::KafkaClientConfig`).
//! - `TOPIC_REPLICATION_FACTOR` — REQUIRED. No default on purpose: silently
//!   defaulting to 1 on a multi-broker cluster would create unreplicated topics.
//! - `TOPIC_PARTITIONS` — partitions per topic (default 12, matching the KEDA
//!   workers' `maxReplicaCount` cap: a consumer group cannot parallelize beyond
//!   its partitions).

use std::time::Duration;

use anyhow::{bail, Context, Result};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::types::RDKafkaErrorCode;
use transport::kafka::config::KafkaClientConfig;
use transport::kafka::DLQ_SUFFIX;

#[tokio::main]
async fn main() -> Result<()> {
    let replication: i32 = std::env::var("TOPIC_REPLICATION_FACTOR")
        .context("TOPIC_REPLICATION_FACTOR is required (no default: silently creating unreplicated topics on a multi-broker cluster is worse than failing)")?
        .parse()
        .context("TOPIC_REPLICATION_FACTOR must be an integer")?;
    let partitions: i32 = match std::env::var("TOPIC_PARTITIONS") {
        Ok(v) => v.parse().context("TOPIC_PARTITIONS must be an integer")?,
        Err(_) => 12,
    };

    let names = topic_names();
    println!(
        "provisioning {} topics (partitions={partitions}, rf={replication})",
        names.len()
    );

    let admin: AdminClient<DefaultClientContext> = KafkaClientConfig::from_env()
        .to_rdkafka()
        .create()
        .context("build Kafka admin client")?;

    let new_topics: Vec<NewTopic> = names
        .iter()
        .map(|name| NewTopic::new(name, partitions, TopicReplication::Fixed(replication)))
        .collect();

    let results = admin
        .create_topics(
            new_topics.iter(),
            &AdminOptions::new().operation_timeout(Some(Duration::from_secs(30))),
        )
        .await
        .context("create_topics admin call")?;

    let (mut created, mut existing, mut failed) = (0u32, 0u32, Vec::new());
    for result in results {
        match result {
            Ok(topic) => {
                created += 1;
                println!("  [created] {topic}");
            }
            Err((topic, RDKafkaErrorCode::TopicAlreadyExists)) => {
                existing += 1;
                println!("  [exists]  {topic}");
            }
            Err((topic, code)) => {
                println!("  [FAILED]  {topic}: {code}");
                failed.push((topic, code));
            }
        }
    }

    println!("done: created={created} existing={existing} failed={}", failed.len());
    if !failed.is_empty() {
        bail!("{} topic(s) failed to provision: {failed:?}", failed.len());
    }
    Ok(())
}

/// The full broker topic set: every registry stream topic + a `.dlq` per
/// consumed topic. Sorted for stable, diffable logs.
fn topic_names() -> Vec<String> {
    let mut names: Vec<String> = event_topology::all_stream_topics()
        .into_iter()
        .map(str::to_owned)
        .chain(
            event_topology::consumed_stream_topics()
                .into_iter()
                .map(|topic| format!("{topic}{DLQ_SUFFIX}")),
        )
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_set_covers_registry_and_dlqs_without_duplicates() {
        let names = topic_names();

        for topic in event_topology::all_stream_topics() {
            assert!(names.contains(&topic.to_owned()), "missing {topic}");
        }
        for topic in event_topology::consumed_stream_topics() {
            let dlq = format!("{topic}{DLQ_SUFFIX}");
            assert!(names.contains(&dlq), "missing {dlq}");
        }

        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped);
    }
}
