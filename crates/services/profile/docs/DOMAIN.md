# `profile` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Profile — the public persona over an account |
> | **Subdomain class** | **Supporting** — the presentation layer of identity; product-facing but derived from `account` |
> | **System of …** | **Record** for public persona (handle, display name, bio, avatar, tier, visibility) |
> | **Aggregate root(s)** | `Profile` (`domain`) |
> | **Tier** | **TIER-0** |
> | **Failure posture** | **Fail-closed on writes** — persona changes (esp. handle) must be consistent |
> | **Upstream contexts** | `account` (account lifecycle); `social-graph` (author tier source); clients (edits) |
> | **Downstream contexts** | `post`, `search`, `geo-discovery`, `timeline` — via **Published Language** (`profile.v1.events`) |
> | **Decision log** | _none yet — see [`docs/adr/`](../../../../docs/adr/README.md)_ |

---

## 1. Business Capability & Non-Goals

**Capability.** `profile` is the authority for **the public persona**: it answers
**"what is this user's handle, display name, bio, avatar, tier, and visibility?"**

**The hard problem.** Owning a globally-unique, claimable **handle** with no conflicts under
concurrency, plus dual-axis visibility (owner choice **and** moderation masking), while being the
denormalization source other read models embed.

**Non-goals — what this context deliberately does NOT do:**
- ❌ Own the account/credentials/PII → `account` is the SoR; profile is the public face.
- ❌ Compute author tier → consumes it (the social-graph→profile tier initiative); profile *owns and emits* it.
- ❌ Build feeds/search → emits `profile.v1.events` those consume.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Profile | The public persona record | `Profile`, `ProfileId` |
| Handle | The globally-unique, claimable @name | `Handle`, `HandleChanged` |
| Display name / bio / avatar / banner | Presentation fields | `DisplayName`, `Bio`, `AvatarUrl`, `BannerUrl` |
| Visibility | Owner-chosen visibility | `ProfileVisibility` |
| Masking reason | Why moderation hid a profile | `MaskingReason`, `ProfileHidden` |
| Verification | Verified-badge state | `VerificationKind`, `ProfileVerified` |
| Tier | The author tier (denormalized + emitted) | `TierChanged` |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `Profile` | aggregate root | Persona consistency + handle uniqueness |
| `Handle` | VO | Global uniqueness; claim under concurrency |
| `DisplayName` / `Bio` / `AvatarUrl` / `BannerUrl` / `WebsiteUrl` / `Locale` | VO | Field validity at construction |
| `ProfileVisibility` / `ProfileStatus` / `ProfileKind` / `VerificationKind` | enum | Closed visibility/status/kind/verification vocabularies |

**Lifecycle:**

```
created --(update / verify)--> active --(hide: owner OR moderation)--> hidden --(restore)--> active --> deleted
```

> **Legal transitions only.** Visibility is **dual-axis** — a profile is visible only if the owner
> *and* moderation both allow it; a handle change is an event, not a silent rename.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth for:**
- The public persona (handle, names, bio, media URLs, tier, visibility) — **ScyllaDB** + **Redis** (entity cache L1). No other service writes persona fields.

**This context holds copies it does NOT own:**

| Copied data | Owned by | Kept fresh via | Staleness tolerance |
|---|---|---|---|
| Account lifecycle state | `account` | `account.v1.events` (DLQ `account.v1.events.dlq`) | eventually consistent |
| Author tier (computed) | `social-graph` (computes) | tier-change flow | eventually consistent |

**The "do-not-write" list:** profile never writes account/PII, and never builds the read models that consume its events.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Handles are globally unique; claims are conflict-free under concurrency | domain + store | `PRF-1xxx` (claim conflict) |
| I2 | Dual-axis visibility — owner AND moderation must both allow | domain | `PRF-1xxx` |
| I3 | A persona change emits the matching `profile.v1.events` | domain (after-save) | — |
| I4 | Concurrent modifications are detected | domain | `PRF-1xxx` (concurrent_modification) |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Provision / update.** Consume `account.v1.events` to provision a persona; client edits mutate the
`Profile` and emit `profile.v1.events` (incl. `HandleChanged`, `ProfileVerified`, `TierChanged`).

**Handle claim.** A claim checks global uniqueness atomically; conflicts return a claim-conflict
error (tracked by `profile.handle.claim.conflict_total`).

**Tier.** `social-graph` computes the tier from follower count → profile owns and emits
`profile.tier_changed` → `geo-discovery`/`timeline` light up (consumer-ready). *(Producer side per the author-tier initiative.)*

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `account` | upstream | ACL | `account.v1.events` | persona provisioning breaks |
| `social-graph` | upstream | Customer/Supplier | tier computation | tier emission breaks |
| `post`/`search`/`geo-discovery`/`timeline` | downstream | Published Language (OHS) | `profile.v1.events` | author snapshots / indexing break |

> **Anti-Corruption Layer:** the `account` event consumer translates account lifecycle into persona
> provisioning.

---

## 8. Domain Events (semantics, not wire)

| Event (`profile.v1.events`) | Means | Emitted when | Who reacts |
|---|---|---|---|
| `profile_created` / `profile_updated` | persona created/edited | command commits | `post`/`search` (snapshots) |
| `handle_changed` | the @handle changed | handle claim | `search` (re-index), embeds |
| `profile_verified` | verification badge changed | verification | `search`, embeds |
| `tier_changed` | author tier changed | tier recompute | `geo-discovery`, `timeline` |
| `profile_hidden` / `profile_restored` / `profile_deleted` | visibility/lifecycle | owner/moderation action | read models (teardown/restore) |

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| Profile is the public persona over the `account` SoR; emits `profile.v1.events` | [`ADR-0014`](../../../../docs/adr/0014-profile-public-persona-with-dual-axis-visibility.md) | Accepted |
| Dual-axis visibility (owner AND moderation) | [`ADR-0014`](../../../../docs/adr/0014-profile-public-persona-with-dual-axis-visibility.md) | Accepted |
| Author tier: social-graph computes → profile owns + emits | _open — author-tier initiative_ | Scoped |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Supporting — the product-facing presentation of identity, derived from `account`.
- **Volatility:** medium — persona fields and verification evolve.
- **Known modeling debt:** the author-tier producer side (social-graph→profile) is scoped, not fully built.
- **Deferred capabilities:** richer verification flows; profile-event indexing rollout to `search`.
