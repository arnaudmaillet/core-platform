# Architecture Decision Records (ADRs)

> An **ADR** captures one architectural decision, the forces behind it, and the alternatives
> rejected — the *why* that code and diagrams cannot express and that otherwise evaporates into
> Slack threads and tribal memory.

## Conventions

- **Format:** MADR-lean — see [`0000-template.md`](./0000-template.md).
- **Naming:** `00NN-<kebab-title>.md`, number zero-padded to 4 digits, monotonically increasing.
- **Immutability:** an Accepted ADR is never edited to change its decision. To reverse it, add a
  **new** ADR that supersedes it and flip the old one's status to `Superseded by ADR-00MM`. The
  preserved trail of superseded decisions is the point.
- **Linkage:** every ADR is referenced from the affected service's `DOMAIN.md §9` (and, for
  cross-context decisions, from `docs/domain/CONTEXT_MAP.md`). Never inline the rationale in those
  docs — link the ADR so the *why* lives in exactly one place.

## Status lifecycle

```
Proposed --> Accepted --> Superseded by ADR-00MM
                      \--> Deprecated
```

## Index

| ADR | Title | Status | Context(s) |
|---|---|---|---|
| [0000](./0000-template.md) | _Template (not a decision)_ | — | — |
| [0001](./0001-audit-is-a-separate-evidence-plane.md) | Audit is a separate tamper-evident evidence plane, not a log aggregator | Accepted | audit |
| [0002](./0002-moderation-decision-enforcement-sor-with-fail-closed-screen.md) | Moderation is the decision/enforcement SoR with a narrow fail-closed Screen gate | Accepted | moderation |
| [0003](./0003-realtime-is-a-fail-open-system-of-connection.md) | Realtime is a fail-open System-of-Connection, never a record store | Accepted | realtime |

<!-- Add one row per ADR as it lands. -->

## Backlog

These three exemplars migrate each service's **keystone** decision. The finer-grained locked
choices behind them are good candidates for their own ADRs as they are next revisited — e.g.
audit's crypto-shred RtbF mechanism, its dual-lane fail-open/fail-closed split, and the
hash-chain + WORM + external-witness immutability scheme could each become a standalone ADR that
this keystone references.
