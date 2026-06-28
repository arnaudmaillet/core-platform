# ADR-0012: Notification uses write-collapse fan-out and a claim-gated idempotent unread counter

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** notification; upstream comment, engagement, post, social-graph
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

A user's activity feed has high fan-in: many events (likes, comments, follows) target one user, and
a naive "one feed write per event" amplifies writes and produces noisy per-event spam ("5 people
liked" should collapse to one entry). At-least-once Kafka delivery also means redelivery must not
double-count the unread badge.

## Decision

Notification is a **derived TIER-2 feed** that **write-collapses** the high-fan-in stream into the
per-user feed (3 layers + an hourly cap), backed by a TWCS activity feed. It guarantees idempotency
with **deterministic UUIDv5 ids**, **event-time `created_at`** (not ingest time), a **claim-gated
unread counter** (`SET NX` so a redelivery can't double-increment), and **unique-sender collapse**
(`SADD`). Live push goes over the gRPC broadcast stream; offline delivery is delegated (APNs/FCM).

## Consequences

- **Positive:** bounded write amplification; collapsed, non-spammy entries; redelivery-safe unread
  counts; missed notifications are re-derivable (fail-open).
- **Negative / accepted trade-off:** collapse logic adds Redis-side complexity; the feed is a
  derived view, authoritative only for itself.
- **Closes:** write amplification and double-counted badges under at-least-once delivery.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| One feed write per source event | Write amplification + noisy per-event spam |
| Increment unread without a claim | Redelivery double-counts the badge |
| Ingest-time `created_at` | Misorders the feed under consumer lag/replay |
