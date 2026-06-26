//! Reconciliation against the owning source-of-record, healing both live tiers.

use std::sync::Arc;

use counter::application::command::ReconcileOutcome;
use counter::domain::{EntityId, EntityKind, EntityRef, Metric};

use super::super::harness::{FixedSource, Harness, at};

fn like(entity: &EntityRef, ms: i64) -> counter::domain::Observation {
    counter::domain::Observation::sum(entity.clone(), Metric::Like, 1, at(ms)).unwrap()
}

fn fresh_profile() -> EntityRef {
    EntityRef::new(
        EntityKind::Profile,
        EntityId::new(format!("profile-{}", uuid::Uuid::now_v7())).unwrap(),
    )
}

#[tokio::test]
async fn corrects_exact_counter_drift_on_both_tiers() {
    let h = Harness::start().await;
    let profile = fresh_profile();

    // The hot/durable Like count drifted to 95 (e.g. a lost window).
    h.ingest((0..95).map(|i| like(&profile, 1_000 + i)).collect()).await;
    assert_eq!(h.total(&profile, Metric::Like).await, Some(95));

    // The authoritative source (engagement) says the true count is 100.
    let source = Arc::new(FixedSource::default());
    source.set(&profile, Metric::Like, 100);

    let outcome = h
        .reconciler(source, 2)
        .reconcile(&profile, Metric::Like)
        .await
        .unwrap();

    assert_eq!(outcome, ReconcileOutcome::Corrected { from: 95, to: 100 });
    // Both the durable ledger (set_total) and the hot counter (overwrite) healed.
    assert_eq!(h.total(&profile, Metric::Like).await, Some(100));
    assert_eq!(h.read(&profile, &[Metric::Like]).await.get(Metric::Like), Some(100));
}
