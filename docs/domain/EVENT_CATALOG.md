# Event Catalog (semantic)

> Populated from each producer's Domain Card §8 (`crates/services/<svc>/docs/DOMAIN.md`). Records the
> **business meaning** of every domain event — *what does it mean that this happened, and who
> reacts?* It does **not** restate the wire/proto schema (owned by each producer's contract).

## Source of truth & maintenance

The list of topics, producers, and consumers is **machine-checked** by the event-topology registry
guard (with its contract tests). The **topic / producer / consumer** columns here should be
**reconciled with that registry** (ideally generated from it) so they cannot drift; only the
**semantic** columns (*means* / *trigger*) are authored by humans. Until generation is wired, treat
the registry guard — not this table — as the authority on *which* edges exist; this table is the
authority on what they *mean*.

Cross-reference each edge in [`CONTEXT_MAP.md`](./CONTEXT_MAP.md), and the per-event detail in each
producer's `DOMAIN.md §8`.

## Identity & Account — `account.v1.events` (producer: `account`)

| Event | Means (past-tense business fact) | Emitted when | Consumers & why |
|---|---|---|---|
| `account_created` / `email_changed` / `email_verified` / `phone_changed` | a PII-bearing account lifecycle fact occurred | the matching command commits | `audit` (PII sealed in crypto-shred envelope), `profile` (persona) |
| `password_changed` / `mfa_enrolled` / `mfa_revoked` | a security/credential fact (no PII) | credential change | `audit` (Authentication category) |
| `activated` / `deactivated` / `suspended` / `deleted` / `kyc_status_changed` | an identity-lifecycle transition | lifecycle change | `audit` (Identity), `profile` |
| `role_assigned` / `role_revoked` | an authorization grant changed | role grant/revoke | `audit` (Authorization) |
| `gdpr_deletion_requested` | the right to erasure (Art. 17) was invoked | user/DPO request | `audit` → **crypto-shreds the subject** (closes the Art. 17 loop) |
| `gdpr_data_export_requested` | the right to access/portability was invoked | user/DPO request | export fulfilment (downstream) |

## Authentication — `auth.v1.events` (producer: `auth`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `session_issued` | an authenticated session was established | login / token issuance | `audit` (Authentication) |
| `session_revoked` | a session was invalidated | logout / revoke / generation bump | `audit` (Authentication) |
| `subject_linked` | an IdP subject was bound to an account | account-link flow | internal |

## Profile — `profile.v1.events` (producer: `profile`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `profile_created` / `profile_updated` | the public persona was created/edited | command commits | `post`/`search` (snapshots, indexing) |
| `handle_changed` | the @handle changed | handle claim | `search` (re-index), embeds |
| `profile_verified` | the verification badge changed | verification | `search`, embeds |
| `tier_changed` | the author tier changed | tier recompute (from `social-graph`) | `geo-discovery` (weight), `timeline` (push/pull) |
| `profile_hidden` / `profile_restored` / `profile_deleted` | a visibility/lifecycle transition | owner or moderation action | read models (teardown/restore) |

## Content — `post.v1.events` (producer: `post`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `post.published` | new content went live | publish commits | `timeline` (fan-out), `search`/`geo-discovery` (index), `counter`, `realtime` (broadcast) |
| `post.updated` | content was edited | update commits | `search`/`geo-discovery` (re-index) |
| `post.deleted` | content was removed | delete commits | `timeline`/`search`/`geo-discovery` (teardown) |

## Comments — `comment.created` / `comment.deleted` (producer: `comment`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `comment.created` | a comment was posted on a post | create commits | `notification` (notify author), `counter`/`engagement` (count++) |
| `comment.deleted` | a comment was tombstoned or purged | delete commits | `counter`/`engagement` (count--), feeds |

## Engagement — `engagement.*` (producer: `engagement`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `engagement.reactions` (`ReactionUpserted`/`Removed`) | a reaction edge was set/cleared | react/unreact commits | `notification`, `counter` |
| `engagement.score_updated` | the weighted engagement score changed | score recompute | `geo-discovery` (virality), `counter` |
| `engagement.post_reactions` / `engagement.post_interaction_counters` | per-post reaction/interaction rollups | aggregation | downstream consumers |

## Magnitudes — `counter.v1.popularity` (producer: `counter`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `counter.v1.popularity` | an entity's popularity magnitude changed | a window flush updates a popularity score | `search` (ranking), `realtime` (live broadcast) |

## Trust & Safety — `moderation.v1.events` (producer: `moderation`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `decision_recorded` | an authoritative integrity ruling was made — carries *who decided* + *why* (DSA SoR) | a decision is recorded (auto-screen / human review / appeal reversal) | `audit` (seals rationale in crypto-shred envelope) |
| `enforcement_applied` / `enforcement_reversed` | a consequence was applied/lifted against an actor (versioned) | enforcement commits | `timeline`, `chat`, `account` (Plane-B denorm); `audit` |
| `case_opened` / `case_resolved` | a review unit opened/closed | ingestion threshold / reviewer action | Plane-B consumers |
| `appeal_resolved` | an appeal was decided | appeal resolution | Plane-B consumers |

> Keyed by `actor_id` for per-actor ordering. `decision_recorded` is the compliance-evidence variant
> (offender-centric consumers ignore it; `audit` consumes it + `enforcement_applied`).

## Conversations — `chat.*` (producer: `chat`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `chat.conversation.created` / `chat.conversation.published` / `chat.conversation.unpublished` | conversation lifecycle facts | create / publish / unpublish | `VisibilityWorker` (audience-plane teardown), consumers |
| `chat.member.joined` / `chat.member.left` | membership changed | join/leave | consumers |
| `chat.message.sent` | a message was committed to the log | send commits | chat's own live plane (**not** consumed by `realtime` — Separate Ways) |

## Media — `media.v1.events` (producer: `media`)

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `asset_uploaded` | bytes landed in the object store | finalize | the transform pipeline (Plane B) |
| `asset_ready` / `asset_variant_ready` | the asset (or a variant) is safe to deliver | CSAM Screen passes / rendition done | `post`, `profile`, `search` |
| `asset_quarantined` / `asset_deleted` / `asset_restored` | a safety/lifecycle transition | Screen fail / takedown / restore | embeds, delivery |
| `asset_failed` | processing failed | timeout/error | upload UX |

## Social Graph — relation events (producer: `social-graph`) — **Kafka producer deferred**

| Event | Means | Emitted when | Consumers & why |
|---|---|---|---|
| `ProfileFollowed` / `ProfileUnfollowed` | a follow edge was created/removed | follow/unfollow commits | `timeline`/`counter` (consume **via gRPC today**; the `social-graph.follows` stream is deferred) |
| `ProfileBlocked` / `ProfileUnblocked` | a block edge changed (severs follows) | block/unblock commits | feeds |
| `AuthorTierChanged` | the author's tier changed | follower count crosses a threshold | `profile` (owns + re-emits as `tier_changed`) |

## Terminal sinks — publish nothing of record

`audit`, `search`, `timeline`, `geo-discovery`, `realtime` consume the above and assert no durable
business facts outward. `notification`'s `NotificationCreated`/`Read` are internal feed state, not a
System-of-Record stream.
