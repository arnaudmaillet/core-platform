# Domain & Functional Documentation

> **Purpose.** This directory holds the **cross-context** functional documentation for the
> platform — the facts that *no single service can keep true on its own*. Per-service domain
> docs live **with their crate**, at `crates/services/<svc>/docs/DOMAIN.md`; this directory only
> holds what spans contexts and indexes the rest.

## The split rule

> **Document each fact where it can become wrong.** If exactly one bounded context can make a
> statement false, it lives in that crate's `DOMAIN.md`. If two or more contexts must agree for
> it to be true, it lives here.

| Lives **with the crate** (`crates/services/<svc>/docs/DOMAIN.md`) | Lives **here** (`docs/domain/`) |
|---|---|
| Bounded context purpose, non-goals | The **context map** (relationships between contexts) |
| Aggregates, invariants, state machines | Cross-context **ubiquitous language** |
| Data ownership (what this context is SoR for) | The semantic **event catalog** |
| Per-context glossary, workflows | This index |

## Ground truth

The source of truth for *what exists* is **the code**, never a diagram:
- the crates — `crates/services/*`, `crates/apps/*`;
- their `*.v1` proto contracts;
- the **event-topology registry guard** (machine-checked, therefore trustworthy).

All documentation here is *derived from* those. The legacy C4 model has been quarantined to
[`docs/_legacy/`](../_legacy/README.md) and **must not be referenced**; a corrected C4 model
will be *regenerated from* the Domain Cards + `CONTEXT_MAP.md`.

## Contents

| File | What it holds | Status |
|---|---|---|
| [`CONTEXT_MAP.md`](./CONTEXT_MAP.md) | DDD context map across all 17 contexts, with relationship patterns (ACL / Conformist / Published Language / OHS / Customer-Supplier / Separate Ways) | ✅ populated |
| [`UBIQUITOUS_LANGUAGE.md`](./UBIQUITOUS_LANGUAGE.md) | Terms shared across more than one context (per-context terms stay in each `DOMAIN.md`) | 🟡 scaffold |
| [`EVENT_CATALOG.md`](./EVENT_CATALOG.md) | Semantic meaning of every domain event (authored from the Domain Cards; topic/producer/consumer columns to be reconciled with the topology guard) | ✅ populated |

## Service domain docs (per-crate `DOMAIN.md`)

The 17 services. Each link will resolve once that service's `docs/DOMAIN.md` lands (see the
rollout in the strategy). Tier drives depth: TIER-0/1 get the full DEEP sections; TIER-2 keeps
CORE plus collapsed one-line DEEP sections.

| Service | Bounded context | `DOMAIN.md` | Status |
|---|---|---|---|
| `account` | Account / Identity SoR | [`crates/services/account/docs/DOMAIN.md`](../../crates/services/account/docs/DOMAIN.md) | ✅ |
| `audit` | Tamper-evident compliance evidence | [`crates/services/audit/docs/DOMAIN.md`](../../crates/services/audit/docs/DOMAIN.md) | ✅ |
| `auth` | Authentication / session / IdP broker | [`crates/services/auth/docs/DOMAIN.md`](../../crates/services/auth/docs/DOMAIN.md) | ✅ |
| `chat` | Conversations & messaging | [`crates/services/chat/docs/DOMAIN.md`](../../crates/services/chat/docs/DOMAIN.md) | ✅ |
| `comment` | Comment threads | [`crates/services/comment/docs/DOMAIN.md`](../../crates/services/comment/docs/DOMAIN.md) | ✅ |
| `counter` | Counter / analytics SoReference | [`crates/services/counter/docs/DOMAIN.md`](../../crates/services/counter/docs/DOMAIN.md) | ✅ |
| `engagement` | Reactions & edge state | [`crates/services/engagement/docs/DOMAIN.md`](../../crates/services/engagement/docs/DOMAIN.md) | ✅ |
| `geo-discovery` | Geo-spatial discovery | [`crates/services/geo-discovery/docs/DOMAIN.md`](../../crates/services/geo-discovery/docs/DOMAIN.md) | ✅ |
| `media` | Media control plane | [`crates/services/media/docs/DOMAIN.md`](../../crates/services/media/docs/DOMAIN.md) | ✅ |
| `moderation` | Trust, safety & integrity | [`crates/services/moderation/docs/DOMAIN.md`](../../crates/services/moderation/docs/DOMAIN.md) | ✅ |
| `notification` | Notification activity feed | [`crates/services/notification/docs/DOMAIN.md`](../../crates/services/notification/docs/DOMAIN.md) | ✅ |
| `post` | Content / posts | [`crates/services/post/docs/DOMAIN.md`](../../crates/services/post/docs/DOMAIN.md) | ✅ |
| `profile` | Public personas | [`crates/services/profile/docs/DOMAIN.md`](../../crates/services/profile/docs/DOMAIN.md) | ✅ |
| `realtime` | Live delivery / connection plane | [`crates/services/realtime/docs/DOMAIN.md`](../../crates/services/realtime/docs/DOMAIN.md) | ✅ |
| `search` | Discovery read-model | [`crates/services/search/docs/DOMAIN.md`](../../crates/services/search/docs/DOMAIN.md) | ✅ |
| `social-graph` | Follower / following relations | [`crates/services/social-graph/docs/DOMAIN.md`](../../crates/services/social-graph/docs/DOMAIN.md) | ✅ |
| `timeline` | Timeline fan-out | [`crates/services/timeline/docs/DOMAIN.md`](../../crates/services/timeline/docs/DOMAIN.md) | ✅ |

## Authoring

- Template: [`docs/templates/DOMAIN.template.md`](../templates/DOMAIN.template.md) — copy to
  `crates/services/<svc>/docs/DOMAIN.md` and fill it.
- Decisions: capture rationale as immutable ADRs under [`docs/adr/`](../adr/README.md) and link
  them from `DOMAIN.md §9` — never inline the *why*.
- i18n: English is canonical; a `DOMAIN.fr.md` mirror follows the
  [translation standard](../i18n/TRANSLATION.md). The drift gate currently scans `README.*.md`
  only; extending it to `DOMAIN.*.md` is part of the rollout's governance phase.

> 🇫🇷 A French mirror (`README.fr.md`) of this index will be added once the scaffold is filled
> with real content, per the translation standard.
