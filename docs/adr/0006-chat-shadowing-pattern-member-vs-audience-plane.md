# ADR-0006: Chat uses the Shadowing Pattern — separate Member and Audience visibility planes

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** chat
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

A conversation has two distinct audiences with different rights: **members** who read and write,
and a broader **audience** who may view a *published* conversation but not participate. Modelling
both with one visibility flag leaks member-only state to the audience (or hides published content
from it), and a published conversation that is later unpublished must have its audience view torn
down cleanly without disturbing the member log.

## Decision

Chat models the two as **separate planes — the Member plane and the Audience plane (the Shadowing
Pattern)**. Membership is authorized at the gRPC boundary for the member plane; publishing opens the
audience plane; unpublishing triggers a `VisibilityWorker` that consumes
`chat.conversation.unpublished` and dismantles audience-plane state. The message log is stored in a
bucketed `(conversation_id, bucket)` ScyllaDB partition, decoupled from visibility.

## Consequences

- **Positive:** member-only state never leaks to the audience; publish/unpublish is a clean plane
  toggle, not a per-message rewrite; the high-volume log scales by bucket.
- **Negative / accepted trade-off:** two planes to keep coherent, and an async teardown worker
  (with its own DLQ) to operate.
- **Closes:** the visibility-conflation leak and the unpublish-teardown problem.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Single visibility flag per conversation | Leaks member state to the audience or hides published content |
| Per-message ACL checks at read | Expensive on a high-volume log; doesn't model the audience plane |
