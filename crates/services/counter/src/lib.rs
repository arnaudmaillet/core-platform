//! `counter` — the platform's **real-time counter aggregator and analytics
//! System-of-Reference**: it absorbs the engagement/view firehose, serves coarse
//! magnitudes back to the fleet at sub-millisecond latency, and owns no entity.
//!
//! This service is a System-of-*Reference*, never a System-of-Record. Every
//! count it holds is a derived materialization of truth that lives elsewhere —
//! reconstructable, at any moment, by replaying the owning services' event
//! streams. That single framing sets its boundaries:
//!
//! * it answers **"how many?"** (views, likes, shares, followers, reach), never
//!   **"who?"** or **"which ones?"** — the per-actor edge state (who liked, who
//!   follows) belongs to `engagement` (reactions) and `social-graph` (follows).
//!   This service counts magnitudes; it cannot tell you whether Alice follows
//!   Bob, only that Bob has 4.2M followers.
//! * the **content/identity services** (`post` / `profile` / `media`) own the
//!   authoritative entity; this service holds only the aggregate counts attached
//!   to a reference.
//! * it **supersedes** `engagement`'s ad-hoc raw counters (the `views`/`shares`
//!   strings and the approximate interaction-counter table): `engagement` keeps
//!   weighted reaction *scoring* and the per-profile reaction *edge state*, and
//!   delegates raw magnitude counting here.
//! * `search` and `timeline` consume the coarse, periodic `counter.v1.popularity`
//!   signal this service publishes — never a per-event count, never a synchronous
//!   call.
//!
//! The architectural commitment is a deliberate split into **two deployables**:
//! a **low-latency read server** (`counter-server`, stateless gRPC over the hot
//! Redis tier) and a **heavy stream worker** (`counter-worker`, the windowed
//! firehose aggregator + durable write-behind + reconciliation + popularity
//! publisher). They scale on different axes and share no failure domain — a
//! worker GC pause can never add latency to a feed-hydration read. The command
//! side is **100% Kafka consumers** (there is no write/increment RPC). Posture is
//! **best-effort, fail-open, eventually-consistent**: a counter outage degrades
//! to stale-but-served counts, it never blocks a like, a follow, or a publish.
//! See `project_counter_analytics_blueprint` for the full design.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `CTR-XXXX` namespace.
//! Phase 2: `domain` (CounterDelta + MetricKind[exact-vs-approximate] + the pure
//! windowed-fold aggregator + HLL/CMS abstractions + popularity derivation) ·
//! Phase 3: `application` + ports (CounterStore / CounterLedger / TimeSeriesStore
//! / SignalPublisher + ingestion & query handlers) · Phase 4: `infrastructure`
//! (Redis hot / Postgres warm-ledger / Scylla TWCS cold / Kafka popularity
//! publisher + event-decode) · Phase 5: `app` (composition roots) + `service`
//! (the two runtime wirings — read server + stream worker).

pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;

pub use error::CounterError;
pub use service::{CounterReadService, CounterWorkerService};
