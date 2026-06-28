# Event Catalog (semantic)

> 🟡 **Scaffold — to be generated.** This catalog records the **business meaning** of every
> domain event on the platform: *what does it mean that this happened, and who reacts?* It does
> **not** restate the wire/proto schema — that is owned by each producer's contract.

## Source: the topology guard, not hand-maintenance

The list of topics, their producers, and their consumers is **machine-checked** by the
event-topology registry guard (with its contract tests). This catalog must be **generated** from
that registry so it cannot drift from reality. Hand-editing the topic list here is forbidden;
only the *semantic* columns (meaning, trigger, reaction) are authored by humans.

## Per event

| Topic | Means (past-tense business fact) | Emitted when (domain trigger) | Producer | Consumers & why |
|---|---|---|---|---|
| `<topic.thing.happened>` | `<the irreversible business fact it asserts>` | `<the domain transition that fires it>` | `<ctx>` | `<ctx>` — `<reaction>` |

## Published Language note

Every topic in this catalog is part of a context's **Open-Host Service / Published Language**: a
schema change is a breaking API change for every consumer listed. Cross-reference each topic's
producer/consumer edge in [`CONTEXT_MAP.md`](./CONTEXT_MAP.md), and the per-event *semantics* in
each producer's `DOMAIN.md §8`.
