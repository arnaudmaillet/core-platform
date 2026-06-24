//! Live-broker integration suite for the generic `run_consumer` runtime (Scenarios A–K).
//!
//! Each test mints an isolated [`TestContext`] (unique topic / `.dlq` / group), drives the
//! runner against the shared single-node broker, and asserts on durable broker state —
//! committed offsets and dead-letter records — polled via `await_until`, never via fixed
//! sleeps. The runner is an infinite loop, so every test spawns it on its own task
//! ([`spawn_runner`]) and aborts it *after* the side-effect under test has been confirmed
//! durable (so an async commit is never lost to a premature drop).
//!
//! Gated behind `--features integration-kafka`; requires a Docker daemon.
#![cfg(feature = "integration-kafka")]

mod harness;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::JoinHandle;

use transport::error::TransportError;
use transport::kafka::EventEnvelope;
use transport::kafka::consumer::{
    ConsumedMessage, KafkaConsumerHandle, ProcessFuture, ProcessOutcome, RetryPolicy, run_consumer,
};
use transport::kafka::envelope::ConsumablePayload;
use transport::kafka::producer::KafkaProducerHandle;

use harness::{TestContext, await_until};

/// Upper bound on how long an assertion waits for a durable side-effect to appear.
const WAIT: Duration = Duration::from_secs(10);
/// Poll cadence inside `await_until` — well above broker round-trip, well below `WAIT`.
const POLL: Duration = Duration::from_millis(100);
/// Idle window between successive dead-letter records once the first has arrived.
const DLQ_DRAIN_IDLE: Duration = Duration::from_secs(3);

// ── Test payload ────────────────────────────────────────────────────────────────────

/// Minimal domain payload. Valid records are JSON of this; poison records are raw
/// non-JSON bytes that fail to decode into it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEvent {
    id: u32,
}

// ── Runner driver & producers ─────────────────────────────────────────────────────────

/// Identity coercion that pins a closure literal to the higher-ranked
/// `for<'a> Fn(&'a T) -> ProcessFuture<'a>` shape `run_consumer` requires. A `let`-bound
/// closure that ignores its event is otherwise inferred with a single concrete lifetime,
/// which the compiler then rejects against the HRTB bound; routing it through this
/// expected-typed parameter forces the higher-ranked inference.
fn classify<T, F>(process: F) -> F
where
    F: for<'a> Fn(&'a T) -> ProcessFuture<'a> + Send + 'static,
{
    process
}

/// Spawns `run_consumer` on its own task with fully owned handles, so the future is
/// `Send + 'static`. The caller aborts (or awaits, for Scenario J) the returned handle.
fn spawn_runner<T, F>(
    consumer: KafkaConsumerHandle,
    producer: KafkaProducerHandle,
    policy: RetryPolicy,
    process: F,
) -> JoinHandle<Result<(), TransportError>>
where
    T: ConsumablePayload,
    F: for<'a> Fn(&'a T) -> ProcessFuture<'a> + Send + 'static,
{
    tokio::spawn(async move { run_consumer::<T, F>(&consumer, &producer, &policy, process).await })
}

/// Publishes a well-formed `TestEvent`, awaiting the broker ack so produce order — and
/// therefore offset order — is deterministic regardless of in-flight settings.
async fn produce_valid(producer: &KafkaProducerHandle, topic: &str, key: &str, id: u32) {
    producer
        .publish(EventEnvelope::new(topic, key, TestEvent { id }))
        .await
        .expect("failed to publish valid event");
}

/// Publishes raw non-JSON bytes that the typed consumer cannot decode (a poison record).
async fn produce_poison(producer: &KafkaProducerHandle, topic: &str, key: &str) {
    producer
        .publish_raw(topic, key, POISON_BYTES, HashMap::new())
        .await
        .expect("failed to publish poison record");
}

/// The exact bytes of a poison record, asserted verbatim on the dead-letter side.
const POISON_BYTES: &[u8] = b"not-json";

// ── Observation helpers ───────────────────────────────────────────────────────────────

/// Waits until the group's committed offset on partition 0 reaches `at_least`, or `WAIT`
/// elapses. Returns the observed offset, or `None` on timeout.
async fn await_commit(ctx: &TestContext, at_least: i64) -> Option<i64> {
    await_until(WAIT, POLL, move || async move {
        ctx.committed_offset(0).await.filter(|o| *o >= at_least)
    })
    .await
}

/// Drains up to `max` dead-letter records with a single consumer. The first record gets a
/// long budget (covering the consumer-group join), later records only a short idle window.
async fn drain_dlq(ctx: &TestContext, max: usize) -> Vec<ConsumedMessage<Value>> {
    let handle = ctx.dlq_consumer();
    let mut stream = handle.stream::<Value>();
    let mut out = Vec::new();
    while out.len() < max {
        let budget = if out.is_empty() { WAIT } else { DLQ_DRAIN_IDLE };
        match tokio::time::timeout(budget, stream.next()).await {
            Ok(Some(Ok(msg))) => out.push(msg),
            _ => break,
        }
    }
    out
}

/// The high watermark (next offset) of the dead-letter partition. Read straight from the
/// broker via `fetch_watermarks` — no consumer-group join — so it is instant and race-free,
/// making it a reliable emptiness probe (`high == 0` ⇒ nothing was ever dead-lettered).
async fn dlq_high_watermark(ctx: &TestContext) -> i64 {
    let brokers = ctx.brokers.clone();
    let topic = ctx.dlq_topic.clone();
    tokio::task::spawn_blocking(move || {
        let consumer: BaseConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "watermark-probe")
            .create()
            .expect("failed to build the watermark-probe consumer");
        let (_low, high) = consumer
            .fetch_watermarks(&topic, 0, Duration::from_secs(5))
            .expect("failed to fetch DLQ watermarks");
        high
    })
    .await
    .expect("the watermark-probe task panicked")
}

/// Asserts no record was ever dead-lettered, via the broker watermark (race-free).
async fn assert_dlq_empty(ctx: &TestContext) {
    assert_eq!(
        dlq_high_watermark(ctx).await,
        0,
        "expected no dead-letter records",
    );
}

/// Reads the next record from a fresh consumer in the context group (used to prove
/// redelivery of an uncommitted record in Scenario J).
async fn read_one<T: ConsumablePayload>(
    ctx: &TestContext,
    timeout: Duration,
) -> Option<ConsumedMessage<T>> {
    let handle = ctx.consumer();
    let mut stream = handle.stream::<T>();
    match tokio::time::timeout(timeout, stream.next()).await {
        Ok(Some(Ok(msg))) => Some(msg),
        _ => None,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_millis() as i64
}

/// Asserts the full set of `x-dlq-*` diagnostic headers on an evacuated record.
fn assert_dlq_headers(
    msg: &ConsumedMessage<Value>,
    ctx: &TestContext,
    expected_offset: &str,
    expected_reason: &str,
    expected_attempts: &str,
) {
    let h = &msg.headers;
    assert_eq!(
        h.get("x-dlq-origin-topic").map(String::as_str),
        Some(ctx.topic.as_str()),
        "origin topic header",
    );
    assert_eq!(
        h.get("x-dlq-partition").map(String::as_str),
        Some("0"),
        "partition header",
    );
    assert_eq!(
        h.get("x-dlq-offset").map(String::as_str),
        Some(expected_offset),
        "offset header",
    );
    assert_eq!(
        h.get("x-dlq-reason").map(String::as_str),
        Some(expected_reason),
        "reason header",
    );
    assert_eq!(
        h.get("x-dlq-attempts").map(String::as_str),
        Some(expected_attempts),
        "attempts header",
    );
    assert!(
        h.get("x-dlq-error").is_some_and(|s| !s.is_empty()),
        "error header must be present and non-empty",
    );
    let failed_at: i64 = h
        .get("x-dlq-failed-at-ms")
        .expect("failed-at header present")
        .parse()
        .expect("failed-at header parses as i64");
    let now = now_ms();
    assert!(
        failed_at > 0 && failed_at <= now && now - failed_at < 60_000,
        "failed-at timestamp {failed_at} should be recent (now {now})",
    );
}

// ── Scenario A: happy path ────────────────────────────────────────────────────────────

#[tokio::test]
async fn scenario_a_happy_path_commits_every_offset() {
    let ctx = TestContext::new().await;
    let count = Arc::new(AtomicUsize::new(0));

    let c = count.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            ProcessOutcome::Done
        })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    let seed = ctx.producer();
    for id in 0..3 {
        produce_valid(&seed, &ctx.topic, "k", id).await;
    }

    let committed = await_commit(&ctx, 3).await;
    task.abort();

    assert_eq!(committed, Some(3), "all three offsets committed");
    assert_eq!(
        count.load(Ordering::SeqCst),
        3,
        "each event processed exactly once",
    );
    assert_dlq_empty(&ctx).await;
}

// ── Scenario B: poison/decode record is evacuated without stalling the partition ──────

#[tokio::test]
async fn scenario_b_poison_record_dead_lettered_without_stall() {
    let ctx = TestContext::new().await;
    let count = Arc::new(AtomicUsize::new(0));

    let c = count.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            ProcessOutcome::Done
        })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    let seed = ctx.producer();
    produce_poison(&seed, &ctx.topic, "k").await; // offset 0
    produce_valid(&seed, &ctx.topic, "k", 42).await; // offset 1

    let committed = await_commit(&ctx, 2).await;
    task.abort();

    assert_eq!(
        committed,
        Some(2),
        "partition advanced past the poison record"
    );
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "the valid record was still processed (no head-of-line stall)",
    );

    let dlq = drain_dlq(&ctx, 1).await;
    assert_eq!(dlq.len(), 1, "exactly one record dead-lettered");
    assert_eq!(
        dlq[0].raw_payload, POISON_BYTES,
        "the original poison bytes are preserved verbatim",
    );
    assert_dlq_headers(&dlq[0], &ctx, "0", "decode", "0");
}

// ── Scenario C: permanent reject is dead-lettered with no retry ───────────────────────

#[tokio::test]
async fn scenario_c_permanent_reject_no_retry() {
    let ctx = TestContext::new().await;
    let count = Arc::new(AtomicUsize::new(0));

    let c = count.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            ProcessOutcome::Reject("permanent".into())
        })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    produce_valid(&ctx.producer(), &ctx.topic, "k", 1).await;

    let committed = await_commit(&ctx, 1).await;
    task.abort();

    assert_eq!(committed, Some(1), "rejected record's offset committed");
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "a permanent reject is processed exactly once (never retried)",
    );

    let dlq = drain_dlq(&ctx, 1).await;
    assert_eq!(dlq.len(), 1);
    assert_dlq_headers(&dlq[0], &ctx, "0", "reject", "1");
}

// ── Scenario D: transient failure recovers in place ───────────────────────────────────

#[tokio::test]
async fn scenario_d_transient_then_recovery() {
    let ctx = TestContext::new().await;
    let calls = Arc::new(AtomicUsize::new(0));

    let cc = calls.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let cc = cc.clone();
        Box::pin(async move {
            let n = cc.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                ProcessOutcome::Retry("transient".into())
            } else {
                ProcessOutcome::Done
            }
        })
    });

    let policy = RetryPolicy {
        max_attempts: 5,
        base_backoff: Duration::from_millis(20),
        max_backoff: Duration::from_secs(1),
    };

    let task = spawn_runner::<TestEvent, _>(ctx.consumer(), ctx.producer(), policy, process);
    produce_valid(&ctx.producer(), &ctx.topic, "k", 1).await;

    let committed = await_commit(&ctx, 1).await;
    task.abort();

    assert_eq!(committed, Some(1), "recovered record's offset committed");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "two transient failures then success = three attempts",
    );
    assert_dlq_empty(&ctx).await;
}

// ── Scenario E: retry exhaustion dead-letters ─────────────────────────────────────────

#[tokio::test]
async fn scenario_e_retry_exhaustion_dead_letters() {
    let ctx = TestContext::new().await;
    let calls = Arc::new(AtomicUsize::new(0));

    let cc = calls.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let cc = cc.clone();
        Box::pin(async move {
            cc.fetch_add(1, Ordering::SeqCst);
            ProcessOutcome::Retry("always".into())
        })
    });

    let policy = RetryPolicy {
        max_attempts: 3,
        base_backoff: Duration::from_millis(20),
        max_backoff: Duration::from_secs(1),
    };

    let task = spawn_runner::<TestEvent, _>(ctx.consumer(), ctx.producer(), policy, process);
    produce_valid(&ctx.producer(), &ctx.topic, "k", 1).await;

    let committed = await_commit(&ctx, 1).await;
    task.abort();

    assert_eq!(committed, Some(1), "exhausted record's offset committed");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "exactly max_attempts processing attempts",
    );

    let dlq = drain_dlq(&ctx, 1).await;
    assert_eq!(dlq.len(), 1);
    assert_dlq_headers(&dlq[0], &ctx, "0", "retry-exhausted", "3");
}

// ── Scenario F: backoff respects the exponential + equal-jitter envelope ───────────────

#[tokio::test]
async fn scenario_f_backoff_jitter_envelope() {
    let ctx = TestContext::new().await;
    let stamps = Arc::new(Mutex::new(Vec::<Instant>::new()));

    let s = stamps.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let s = s.clone();
        Box::pin(async move {
            s.lock().expect("stamps lock").push(Instant::now());
            ProcessOutcome::Retry("transient".into())
        })
    });

    let base_ms: u64 = 200;
    let policy = RetryPolicy {
        max_attempts: 4,
        base_backoff: Duration::from_millis(base_ms),
        max_backoff: Duration::from_secs(30),
    };

    let task = spawn_runner::<TestEvent, _>(ctx.consumer(), ctx.producer(), policy, process);
    produce_valid(&ctx.producer(), &ctx.topic, "k", 1).await;

    let committed = await_commit(&ctx, 1).await;
    task.abort();
    assert_eq!(committed, Some(1));

    let stamps = stamps.lock().expect("stamps lock").clone();
    assert_eq!(stamps.len(), 4, "four attempts before exhaustion");

    // Three inter-attempt gaps; gap n follows backoff_for(n+1) = base·2^n, equal-jittered
    // into [interval/2, interval]. Tight lower bound (a sleep never undershoots, modulo
    // timer granularity); generous upper slack for scheduling + per-attempt overhead.
    let gaps: Vec<Duration> = stamps.windows(2).map(|w| w[1] - w[0]).collect();
    for (n, gap) in gaps.iter().enumerate() {
        let interval_ms = base_ms * (1u64 << n);
        let low = Duration::from_millis(interval_ms / 2).saturating_sub(Duration::from_millis(15));
        let high = Duration::from_millis(interval_ms) + Duration::from_millis(300);
        assert!(
            *gap >= low && *gap <= high,
            "gap {n} = {gap:?} outside jitter window [{low:?}, {high:?}]",
        );
    }
}

// ── Scenario G: interleaved poison/valid — ordering and no stall ──────────────────────

#[tokio::test]
async fn scenario_g_interleaved_poison_and_valid() {
    let ctx = TestContext::new().await;
    let count = Arc::new(AtomicUsize::new(0));

    let c = count.clone();
    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            ProcessOutcome::Done
        })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    let seed = ctx.producer();
    produce_valid(&seed, &ctx.topic, "k", 0).await; // offset 0
    produce_poison(&seed, &ctx.topic, "k").await; // offset 1
    produce_valid(&seed, &ctx.topic, "k", 2).await; // offset 2
    produce_poison(&seed, &ctx.topic, "k").await; // offset 3
    produce_valid(&seed, &ctx.topic, "k", 4).await; // offset 4

    let committed = await_commit(&ctx, 5).await;
    task.abort();

    assert_eq!(committed, Some(5), "committed up to the high-water mark");
    assert_eq!(
        count.load(Ordering::SeqCst),
        3,
        "all three valid records processed",
    );

    let dlq = drain_dlq(&ctx, 2).await;
    assert_eq!(dlq.len(), 2, "both poison records dead-lettered");
    assert!(
        dlq.iter()
            .all(|m| m.headers.get("x-dlq-reason").map(String::as_str) == Some("decode")),
        "both evacuations classified as decode failures",
    );
}

// ── Scenario H: dead-letter diagnostic header completeness ────────────────────────────

#[tokio::test]
async fn scenario_h_dlq_diagnostic_headers_complete() {
    let ctx = TestContext::new().await;

    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        Box::pin(async move { ProcessOutcome::Reject("malformed-invariant".into()) })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    produce_valid(&ctx.producer(), &ctx.topic, "k", 99).await;

    await_commit(&ctx, 1).await;
    task.abort();

    let dlq = drain_dlq(&ctx, 1).await;
    assert_eq!(dlq.len(), 1);
    // Full coordinate fidelity: origin topic, partition 0, offset 0, reason, attempts, and
    // a present non-empty error string plus a recent failed-at timestamp.
    assert_dlq_headers(&dlq[0], &ctx, "0", "reject", "1");
}

// ── Scenario I: dead-letter preserves the source key (replay affinity) ────────────────

#[tokio::test]
async fn scenario_i_dlq_preserves_source_key() {
    let ctx = TestContext::new().await;
    let key = "user-42";

    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        Box::pin(async move { ProcessOutcome::Reject("permanent".into()) })
    });

    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.producer(),
        RetryPolicy::default(),
        process,
    );

    produce_valid(&ctx.producer(), &ctx.topic, key, 7).await;

    await_commit(&ctx, 1).await;
    task.abort();

    let dlq = drain_dlq(&ctx, 1).await;
    assert_eq!(dlq.len(), 1);
    assert_eq!(
        dlq[0].key, key,
        "dead-letter record retains the source key for partition affinity on replay",
    );
}

// ── Scenario J: a failed dead-letter publish withholds the commit (at-least-once) ─────

#[tokio::test]
async fn scenario_j_dlq_publish_failure_withholds_commit() {
    let ctx = TestContext::new().await;

    let process = classify::<TestEvent, _>(move |_e: &TestEvent| -> ProcessFuture<'_> {
        Box::pin(async move { ProcessOutcome::Reject("permanent".into()) })
    });

    // Healthy source consumer, but the DLQ producer points at an unroutable broker.
    let task = spawn_runner::<TestEvent, _>(
        ctx.consumer(),
        ctx.broken_producer(),
        RetryPolicy::default(),
        process,
    );

    produce_valid(&ctx.producer(), &ctx.topic, "k", 7).await;

    // The runner must surface the publish failure as Err and return.
    let result = tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .expect("run_consumer did not return within the deadline")
        .expect("run_consumer task panicked");
    assert!(
        result.is_err(),
        "a failed dead-letter publish must surface as Err",
    );

    // The offset must not have advanced.
    assert_eq!(
        ctx.committed_offset(0).await,
        None,
        "no commit on a failed evacuation",
    );

    // And the uncommitted record must be redelivered to a fresh consumer in the group.
    let msg = read_one::<TestEvent>(&ctx, WAIT)
        .await
        .expect("uncommitted record must be redelivered");
    assert_eq!(msg.offset, 0, "redelivered from the last committed offset");
    assert_eq!(
        msg.payload.expect("redelivered payload decodes").id,
        7,
        "the same record is redelivered",
    );
}

// ── Scenario K: mid-stream broker fault (stretch) ─────────────────────────────────────

#[tokio::test]
#[ignore = "stretch: a mid-stream broker fault needs a dedicated container; the \
            no-commit-on-error branch is already proven deterministically by Scenario J"]
async fn scenario_k_broker_stream_error_no_commit() {
    // Per the approved blueprint, a true broker-level stream error is folded into
    // Scenario J: both exercise the same `Err`-without-commit branch of `run_consumer`.
    // Killing the shared broker here would destabilise every other namespaced test in
    // this binary, so a faithful version belongs in a dedicated single-test container.
}
