# ADR-0013: Post uses a two-table ScyllaDB layout and is the published-language fan-out source

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** post; downstream timeline, search, geo-discovery, counter, realtime
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Posts are high-write content read two ways — by post id and by author — and the entire read side of
the platform (feed, search, discovery, counts, live) depends on knowing when content changes. A
single-access store can't serve both reads cheaply, and if consumers reach into post's store the
whole fleet couples to its schema.

## Decision

`post` is the **content System of Record** in a **two-table ScyllaDB layout** (`post.posts` by id +
`post.posts_by_profile` by author), and emits **`post.v1.events`** (`published`/`updated`/`deleted`)
as its **Open-Host Service / Published Language** — the single fan-out source the read side consumes.
Proto enums map the domain tinyint **+1 with no UNSPECIFIED sentinel**. Post denormalizes author and
moderation state by consuming `profile.v1.events` / `moderation.v1.events`.

## Consequences

- **Positive:** both read patterns are cheap; the read side is decoupled (events, not store
  reach-in); one authoritative content lifecycle drives feed/search/discovery/counts/live.
- **Negative / accepted trade-off:** dual-table write consistency to maintain; `post.v1.events` is a
  hard API contract — a schema change breaks every consumer.
- **Closes:** the read-pattern mismatch and read-side coupling to post's store.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Single posts table | Can't serve by-id and by-author reads efficiently |
| Let consumers read post's store | Couples the whole read side to post's schema |
| Synchronous fan-out to consumers | Puts the read side on the write path |
