# ADR-0014: Profile is the public persona over the account SoR with dual-axis visibility

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** profile; account; downstream post, search, geo-discovery, timeline
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

The public face of a user (handle, display name, bio, avatar, tier) is a different concern from the
private account of record (credentials, PII, KYC). Bundling them puts PII in the read-hot persona
store. Two separate parties can also legitimately hide a profile — the **owner** (privacy) and
**moderation** (enforcement) — and a single visibility flag can't represent both without one
silently overriding the other. Handles must be globally unique under concurrent claims.

## Decision

`profile` is the **public-persona System of Record**, layered over the `account` SoR (provisioned by
consuming `account.v1.events`) and holding no PII. Visibility is **dual-axis**: a profile is visible
only if the **owner AND moderation** both allow it. Handles are **globally unique with conflict-free
claims** under concurrency. Profile **owns and emits** `profile.v1.events` (including
`handle_changed`, `profile_verified`, and `tier_changed`) for the read side; the author **tier is
computed by `social-graph`** but owned and published here.

## Consequences

- **Positive:** PII stays in `account`; persona reads are hot and PII-free; both hide-authorities are
  represented independently; downstream read models react via events.
- **Negative / accepted trade-off:** persona must stay eventually consistent with account; the
  handle-claim path needs atomic uniqueness handling.
- **Closes:** PII-in-the-persona-store and the single-visibility-flag override problem.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| One service for account + profile | Puts PII in the read-hot persona store |
| Single visibility flag | Owner and moderation hides silently override each other |
| Compute/own tier in profile | Tier derives from the follower graph (`social-graph`) |
