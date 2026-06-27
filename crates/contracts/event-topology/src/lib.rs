//! Fleet Kafka topic topology — the single source of truth for which service
//! **produces** and which **consumes** every Kafka stream, plus the contract
//! test that fails the build on a *phantom edge*: a consumer subscribing to a
//! topic that no producer emits.
//!
//! ## Why this crate exists
//!
//! Kafka payloads are JSON and topic names are bare string literals scattered
//! across `const TOPIC: &str = "…"` declarations. A misnamed topic or a
//! never-built producer therefore **does not fail to compile** — the consumer
//! just sits idle forever, or poison-DLQs every message. `buf breaking` guards
//! the gRPC *proto* contracts; nothing guarded the *event-streaming* contracts.
//! This registry does.
//!
//! Three real phantom edges (geo-discovery `tier_sync` on a never-produced
//! `profile.tier_changed`, geo-discovery `score_updater` on a never-produced
//! `engagement.score_updated`, and the half-migrated `post.published` family)
//! shipped silently because there was no such gate. This catches the next one.
//!
//! ## How to keep it honest
//!
//! When you add or remove a Kafka producer or consumer **in the same PR**:
//!
//! - add/remove its edge in [`PRODUCERS`] or [`CONSUMERS`];
//! - if a consumer subscribes to a topic whose producer is intentionally not
//!   built in-repo (external system, or a documented deferral), list the topic
//!   in [`DEFERRED`] **with a reason** — otherwise the test fails;
//! - if a producer's topic has no in-repo consumer (intentional headroom), list
//!   it in [`ORPHAN_PRODUCERS`] — otherwise the test fails.
//!
//! The tests reject *stale* entries too: a `DEFERRED` topic that someone has
//! since wired a producer for, or an `ORPHAN_PRODUCERS` topic that now has a
//! consumer, both fail — so the registry can't rot into a pile of excuses.
//!
//! NB: this guards topic **names and wiring**, not payload **shapes**. A
//! producer and consumer can agree on a topic and still disagree on the JSON
//! body (see the post→geo/notification payload gap). Shape compatibility is a
//! separate concern (per-stream wire-lock tests live with each service).

/// Every Kafka topic that some fleet service emits, paired with its owning
/// (producing) service. A topic should have exactly one producer service.
pub const PRODUCERS: &[(&str, &str)] = &[
    // account
    ("account.v1.events", "account"),
    // profile
    ("profile.v1.events", "profile"),
    // post — legacy per-type topics + the unified v1 stream (mid-migration)
    ("post.published", "post"),
    ("post.updated", "post"),
    ("post.deleted", "post"),
    ("post.v1.events", "post"),
    // comment
    ("comment.created", "comment"),
    ("comment.deleted", "comment"),
    // engagement
    ("engagement.reactions", "engagement"),
    // social-graph
    ("social-graph.followed", "social-graph"),
    ("social-graph.unfollowed", "social-graph"),
    ("social-graph.blocked", "social-graph"),
    ("social-graph.author_tier_changed", "social-graph"),
    // chat (its own delivery plane)
    ("chat.conversation.created", "chat"),
    ("chat.conversation.published", "chat"),
    ("chat.conversation.unpublished", "chat"),
    ("chat.member.joined", "chat"),
    ("chat.member.left", "chat"),
    ("chat.message.sent", "chat"),
    // counter
    ("counter.v1.popularity", "counter"),
    // moderation
    ("moderation.v1.events", "moderation"),
    // auth
    ("auth.v1.events", "auth"),
    // media
    ("media.v1.events", "media"),
];

/// Every Kafka subscription in the fleet, paired with its consuming service.
/// A topic may have several consumers.
pub const CONSUMERS: &[(&str, &str)] = &[
    // account lifecycle → compliance plane + profile projection
    ("account.v1.events", "audit"),
    ("account.v1.events", "profile"),
    // profile lifecycle → search index + post author-tier denormalization
    ("profile.v1.events", "search"),
    ("profile.v1.events", "post"),
    // post (legacy)
    ("post.published", "notification"),
    ("post.published", "geo-discovery"),
    ("post.deleted", "timeline"),
    // post (unified v1)
    ("post.v1.events", "timeline"),
    ("post.v1.events", "search"),
    ("post.v1.events", "realtime"),
    // comment
    ("comment.created", "notification"),
    ("comment.created", "engagement"),
    ("comment.deleted", "engagement"),
    // engagement reactions → counter aggregation, notif fan-out, write-behind
    ("engagement.reactions", "counter"),
    ("engagement.reactions", "notification"),
    ("engagement.reactions", "engagement"),
    // social-graph edges → timeline fan-out, profile tier ownership
    ("social-graph.followed", "timeline"),
    ("social-graph.unfollowed", "timeline"),
    ("social-graph.author_tier_changed", "profile"),
    // chat visibility teardown (self-consume)
    ("chat.conversation.unpublished", "chat"),
    // counter popularity → realtime broadcast + geo virality re-score
    ("counter.v1.popularity", "realtime"),
    ("counter.v1.popularity", "geo-discovery"),
    // auth lifecycle → compliance plane
    ("auth.v1.events", "audit"),
    // moderation decisions → compliance plane, search visibility, media takedown
    ("moderation.v1.events", "audit"),
    ("moderation.v1.events", "search"),
    ("moderation.v1.events", "media"),
    // media lifecycle → Plane-B processing pipeline (self-consume)
    ("media.v1.events", "media"),
    // audit generic ingest lane (see DEFERRED)
    ("audit.v1.events", "audit"),
    // moderation intake lanes (see DEFERRED — external producers)
    ("moderation.reports", "moderation"),
    ("moderation.signals", "moderation"),
    // counter telemetry + follow folds (see DEFERRED — producers not built)
    ("view.v1.events", "counter"),
    ("impression.v1.events", "counter"),
    ("click.v1.events", "counter"),
    ("social-graph.follows", "counter"),
];

/// Topics a consumer subscribes to whose producer is **intentionally** not built
/// in-repo: external systems, or documented roadmap deferrals. Each entry needs
/// a reason. Keeps the phantom-edge test honest without hiding the gap.
pub const DEFERRED: &[(&str, &str)] = &[
    (
        "audit.v1.events",
        "Generic privileged-record ingest lane. Domain producers emit their own \
         topics (account/auth/moderation .v1.events) which audit consumes \
         directly; this lane is fed by the sync gRPC RecordPrivileged path and \
         future generic producers.",
    ),
    (
        "moderation.reports",
        "External user-report intake — produced by the client/edge, not a fleet \
         service.",
    ),
    (
        "moderation.signals",
        "External ML-classifier signals — produced off-fleet.",
    ),
    (
        "view.v1.events",
        "Upstream view telemetry producer not yet built (counter-analytics \
         blueprint deferral).",
    ),
    (
        "impression.v1.events",
        "Upstream impression telemetry producer not yet built (counter \
         deferral).",
    ),
    (
        "click.v1.events",
        "Upstream click telemetry producer not yet built (counter deferral).",
    ),
    (
        "social-graph.follows",
        "Counter wants a single combined follow stream; social-graph emits the \
         split past-tense social-graph.followed/.unfollowed instead. Combined \
         producer is deferred — TRACKED NAMING MISMATCH, not just a missing emitter.",
    ),
];

/// Topics a producer emits that have **no** in-repo consumer: intentional
/// headroom or read-path-enforced concerns. Each entry needs a reason.
pub const ORPHAN_PRODUCERS: &[(&str, &str)] = &[
    (
        "post.updated",
        "No stream consumer — search/timeline/realtime act on post.v1.events \
         PostUpdated; the legacy per-type topic is emitted for completeness.",
    ),
    (
        "social-graph.blocked",
        "Block is enforced on the gRPC read path; no stream consumer yet.",
    ),
    (
        "chat.conversation.created",
        "Chat owns its own delivery plane; reserved for future fan-out.",
    ),
    (
        "chat.conversation.published",
        "Chat delivery-plane headroom.",
    ),
    (
        "chat.member.joined",
        "Chat delivery-plane headroom.",
    ),
    (
        "chat.member.left",
        "Chat delivery-plane headroom.",
    ),
    (
        "chat.message.sent",
        "Future realtime/notification consolidation; chat streams to clients \
         directly today.",
    ),
];

/// The set of valid service names, to catch typos in the tables above.
pub const SERVICES: &[&str] = &[
    "account",
    "profile",
    "social-graph",
    "post",
    "engagement",
    "comment",
    "geo-discovery",
    "notification",
    "timeline",
    "chat",
    "auth",
    "moderation",
    "search",
    "media",
    "counter",
    "realtime",
    "audit",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn produced_topics() -> HashSet<&'static str> {
        PRODUCERS.iter().map(|(t, _)| *t).collect()
    }
    fn consumed_topics() -> HashSet<&'static str> {
        CONSUMERS.iter().map(|(t, _)| *t).collect()
    }
    fn deferred_topics() -> HashSet<&'static str> {
        DEFERRED.iter().map(|(t, _)| *t).collect()
    }
    fn orphan_topics() -> HashSet<&'static str> {
        ORPHAN_PRODUCERS.iter().map(|(t, _)| *t).collect()
    }

    /// The headline guard: nothing may consume a topic that no producer emits,
    /// unless the gap is explicitly deferred with a reason.
    #[test]
    fn every_consumed_topic_has_a_producer_or_is_deferred() {
        let produced = produced_topics();
        let deferred = deferred_topics();

        let phantom: Vec<_> = consumed_topics()
            .into_iter()
            .filter(|t| !produced.contains(t) && !deferred.contains(t))
            .collect();

        assert!(
            phantom.is_empty(),
            "PHANTOM CONSUMER EDGE(S): these topics are consumed but no service \
             produces them and they are not in DEFERRED — either wire a producer, \
             fix the topic name, or add a DEFERRED entry with a reason: {phantom:?}"
        );
    }

    /// A deferral that has since been fulfilled (a producer now emits it) must be
    /// removed from DEFERRED so the registry stays truthful.
    #[test]
    fn no_deferred_topic_is_actually_produced() {
        let produced = produced_topics();
        let stale: Vec<_> = deferred_topics()
            .into_iter()
            .filter(|t| produced.contains(t))
            .collect();

        assert!(
            stale.is_empty(),
            "STALE DEFERRAL: these topics now have a producer — remove them from \
             DEFERRED: {stale:?}"
        );
    }

    /// Every produced-but-unconsumed topic must be acknowledged as an intentional
    /// orphan; a *new* orphan (someone added a producer with no consumer and no
    /// allowlist entry) fails the build.
    #[test]
    fn every_produced_topic_is_consumed_or_an_acknowledged_orphan() {
        let consumed = consumed_topics();
        let orphans = orphan_topics();

        let unaccounted: Vec<_> = produced_topics()
            .into_iter()
            .filter(|t| !consumed.contains(t) && !orphans.contains(t))
            .collect();

        assert!(
            unaccounted.is_empty(),
            "UNCONSUMED PRODUCER: these topics are produced but nobody consumes \
             them and they are not in ORPHAN_PRODUCERS — wire a consumer or \
             acknowledge the orphan with a reason: {unaccounted:?}"
        );
    }

    /// An orphan that has since gained a consumer must leave the allowlist.
    #[test]
    fn no_orphan_producer_is_actually_consumed() {
        let consumed = consumed_topics();
        let stale: Vec<_> = orphan_topics()
            .into_iter()
            .filter(|t| consumed.contains(t))
            .collect();

        assert!(
            stale.is_empty(),
            "STALE ORPHAN: these topics now have a consumer — remove them from \
             ORPHAN_PRODUCERS: {stale:?}"
        );
    }

    /// Topic ownership is singular: exactly one service may produce a topic.
    #[test]
    fn each_topic_has_a_single_producer() {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut dup: Vec<&str> = Vec::new();
        for (topic, _service) in PRODUCERS {
            if !seen.insert(topic) {
                dup.push(topic);
            }
        }
        assert!(
            dup.is_empty(),
            "DUPLICATE PRODUCER: these topics are claimed by more than one \
             producer service: {dup:?}"
        );
    }

    /// A topic can't be both a documented deferral and a documented orphan.
    #[test]
    fn deferred_and_orphan_sets_are_disjoint() {
        let overlap: Vec<_> = deferred_topics()
            .intersection(&orphan_topics())
            .copied()
            .collect();
        assert!(
            overlap.is_empty(),
            "A topic is in both DEFERRED and ORPHAN_PRODUCERS: {overlap:?}"
        );
    }

    /// Every service name in the edge tables must be a known service (typo guard).
    #[test]
    fn all_referenced_services_are_known() {
        let known: HashSet<&str> = SERVICES.iter().copied().collect();
        let unknown: Vec<_> = PRODUCERS
            .iter()
            .chain(CONSUMERS.iter())
            .map(|(_, s)| *s)
            .filter(|s| !known.contains(s))
            .collect();
        assert!(
            unknown.is_empty(),
            "UNKNOWN SERVICE NAME in registry (typo?): {unknown:?}"
        );
    }

    /// DEFERRED and ORPHAN_PRODUCERS reasons must be non-empty — no silent excuses.
    #[test]
    fn every_exception_carries_a_reason() {
        for (topic, reason) in DEFERRED.iter().chain(ORPHAN_PRODUCERS.iter()) {
            assert!(
                !reason.trim().is_empty(),
                "topic {topic:?} is excepted without a reason"
            );
        }
    }
}
