# ADR-0017: Timeline uses a hybrid push/pull fan-out

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** timeline; upstream post, social-graph, profile
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Feed generation has two classic failure modes. **Fan-out on write** (materialize every post into
every follower's feed) explodes for high-follower authors — one celebrity post writes millions of
rows. **Fan-out on read** (pull every followee's posts at read time) explodes for users who follow
many accounts. Neither extreme scales for both author and reader distributions.

## Decision

`timeline` is a **SoReference feed read-model** using a **hybrid push/pull fan-out**: normal-tier
authors are **pushed** (materialized into follower feed ZSETs on `post.published`); high-tier authors
are **pulled** at read time, and the two are **merged via a Lua `ZREVRANGEBYSCORE`**, paginated by
`FeedCursor`. The push/pull boundary is driven by `AuthorTier` (consumed from `profile`). The follower
set is read from `social-graph` over gRPC; the feed is fail-open and rebuildable.

## Consequences

- **Positive:** bounds both write amplification (no million-row celebrity fan-out) and read
  amplification (no pull of thousands of followees); ordering is a single Lua merge.
- **Negative / accepted trade-off:** read-time merge complexity; correctness depends on the
  author-tier signal; fan-out performance is a tracked tuning debt.
- **Closes:** the celebrity write-amplification and the heavy-follower read-amplification problems.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Pure fan-out on write | Celebrity posts write millions of feed rows |
| Pure fan-out on read | Users following many accounts pay huge read cost |
| Static threshold without tier signal | Misclassifies authors; needs the `profile` tier input |
