# `audit` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Compliance Evidence — the tamper-evident audit trail |
> | **Subdomain class** | **Supporting** — legally non-negotiable but not a user-facing differentiator; bespoke (not Generic) because no off-the-shelf SIEM/audit tool reconciles GDPR Art. 17 erasure with Art. 5(2) accountability |
> | **System of …** | **Record / Evidence** for "who did what, to whom, when, under what authority, with what outcome" |
> | **Aggregate root(s)** | `AuditRecord` (`domain::record`), with the per-partition `ChainLink` / `ChainHead` chain (`domain::chain`) and `MerkleCheckpoint` (`domain::checkpoint`) |
> | **Tier** | **TIER-0** |
> | **Failure posture** | **Split** — *fail-open* at producers (audit liveness never browns out the mesh); *fail-closed* on durability and on the synchronous break-glass lane |
> | **Upstream contexts** | `moderation`, `auth`, `account` via **Conformist + ACL** (audit consumes their event streams and translates each foreign wire shape into `AuditEvent` at `infrastructure/decode.rs`) |
> | **Downstream contexts** | none of record — audit is a **terminal sink**; it publishes nothing other services consume |
> | **Decision log** | [`ADR-0001`](../../../../docs/adr/0001-audit-is-a-separate-evidence-plane.md) |

---

## 1. Business Capability & Non-Goals

**Capability.** `audit` is the authority for **compliance evidence**: it answers
**"can we prove this record of who-did-what is complete and was never altered, and would we detect it if it were?"**

**The hard problem.** Telemetry and an audit trail look alike on a screen but are different
substances: telemetry is best-effort, mutable, sampled and retention-cycled; evidence must be
zero-loss, append-only, tamper-evident, complete, retained for years, and erasable at the *field*
level. Conflating them puts PII into uncontrolled log indexes and makes a GDPR Art. 17 erasure
request directly contradict the Art. 5(2) duty to retain proof.

**Non-goals — what this context deliberately does NOT do:**
- ❌ General application logging / observability → owned by the telemetry plane (`telemetry` crate).
- ❌ Making or acting on decisions — audit *records* decisions others make and acts on none.
- ❌ Holding the identity↔pseudonym mapping → owned by `account`; audit only ever sees pseudonyms.
- ❌ Publishing events of record → it is a terminal sink.

---

## 2. Ubiquitous Language

> Cross-context terms (subject, lawful basis) live in `docs/domain/UBIQUITOUS_LANGUAGE.md`.

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Audit record | One immutable, hash-linked entry in the evidence ledger | `AuditRecord` |
| Chain link / head | The `H(prev ‖ canonical(payload) ‖ sequence_no)` link and the current tip of a per-partition chain | `ChainLink`, `ChainHead` |
| Crypto-shred | Erasure by destroying a subject's key, leaving the record + proof intact | via `PiiEnvelope` + `KeyVault` |
| PII envelope | A per-subject, crypto-shreddable ciphertext; the chain hashes the *ciphertext*, never cleartext | `PiiEnvelope` |
| Checkpoint | A signed Merkle root over all partition heads, anchored to an external witness | `MerkleCheckpoint` |
| Legal hold | A lawful-retention override (Art. 17(3)) that suspends erasure | `LegalHold` |
| Privileged action | A must-record-before-permitted action recorded on the synchronous fail-closed lane | `PrivilegedActionType` |
| Partition | The `(tenant, category)` shard a record's chain belongs to; subject is indexed, not the partition key | `EventCategory` |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `AuditRecord` | aggregate root | A record, once committed, is immutable and hash-linked to its predecessor |
| `NewAuditEvent` / `AuditEvent` | entity / VO | A well-formed, deduplicated intake event before/after it is sequenced into the chain |
| `ChainLink` / `ChainHead` | VO | Per-partition append-only linkage; `sequence_no` is strictly monotonic (gap = truncation signal) |
| `MerkleCheckpoint` | VO | A signed root binding all partition heads at a point in time |
| `PiiEnvelope` | VO | PII exists only as crypto-shreddable ciphertext; cleartext never enters the chain |
| `Actor` / `ResourceRef` | VO | The pseudonymous who and the what-was-acted-on |
| `LegalHold` / `RetentionPolicy` | VO | When erasure is permitted vs lawfully suspended |
| `EventCategory` / `ActorType` / `Outcome` / `LawfulBasis` | enum | The closed classification vocabulary |

**Lifecycle of a record:**

```
intake --(dedupe: UUIDv5)--> sequenced (chain-linked) --(persist+archive)--> committed --(checkpoint)--> witnessed
                                                                                  └--(erase: destroy DEK)--> pii-shredded (chain still verifies)
```

> **Legal transitions only.** A committed record is never updated or deleted (`UPDATE`/`DELETE`
> revoked at the DB role). A hash mismatch or sequence gap on read is not a recoverable state — it
> raises `AUD-2001` / `AUD-2002` as a tamper alarm.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth for:**
- The hash-chained compliance ledger — append-only **Postgres** (canonical) mirrored to **Object-Lock WORM** (S3/MinIO, compliance mode). No other service writes it.
- Per-subject DEKs and the signing key custody record (in KMS/HSM, a separate trust domain).

**This context holds copies it does NOT own (read-model / denormalization):**

| Copied data | Owned by | Kept fresh via | Staleness tolerance |
|---|---|---|---|
| Moderation decision facts | `moderation` | `moderation.v1.events` (`decision_recorded`, `enforcement_applied`) | eventually consistent (Kafka lag) |
| Auth session lifecycle | `auth` | `auth.v1.events` (`session_issued`/`session_revoked`) | eventually consistent |
| Account lifecycle + GDPR events | `account` | `account.v1.events` | eventually consistent |

**The "do-not-write" list:** audit never mutates upstream state, never resolves a pseudonym to a
real identity (that mapping lives in `account`), and never emits a record other services depend on.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | PII never enters the chain in cleartext (subjects are pseudonyms; PII is a `PiiEnvelope` ciphertext) | domain + infrastructure | (prevented by construction) |
| I2 | Erasure is key-destruction, not record-deletion — record, sequence, hash and non-PII metadata survive | application | `AUD-5xxx` |
| I3 | A subject under an active legal hold is not shredded (Art. 17(3) overrides) | domain | `AUD-5002` |
| I4 | The most dangerous actions fail closed — `RecordPrivileged` denies unless durably recorded first | application | `AUD-4004` |
| I5 | Reads are evidence too — every query/export is itself recorded; access is need-to-know | application | `AUD-3001`/`AUD-3002` |
| I6 | The chain is append-only and complete; any mutation/gap is detectable | domain + DB role (`UPDATE`/`DELETE` revoked) | `AUD-2001`/`AUD-2002` |
| I7 | A signed checkpoint must reconcile with the external witness | application | `AUD-2004` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Bulk ingestion (async, fail-open).**
1. A fleet service emits a compliance event to Kafka (`audit.v1.events` or a decision stream) — fire-and-forget.
2. `audit-worker` consumes under `run_consumer`; dedupes by UUIDv5 (`AUD-1004` → `Ok`).
3. Decode foreign wire → `AuditEvent`; seal any PII into a `PiiEnvelope`.
4. Append to the per-partition chain → persist to Postgres → mirror to WORM.
- **Commit discipline:** the Kafka offset advances **only after** durable persist + chain. No committed offset ever sits past an un-persisted event → zero loss.
- **Idempotency:** deterministic UUIDv5; redelivery deduped, so each logical event appears once.

**Privileged action (sync, fail-closed).**
1. A privileged caller invokes `RecordPrivileged` on `:50068`.
2. Audit attempts durable+chained commit within a hard deadline.
3. Success → permit; deadline/durability miss → **deny** (`AUD-4004`). The action must not proceed unrecorded.

**Erasure (crypto-shred).** A GDPR Art. 17 request destroys the subject's DEK (unless under legal hold) → all that subject's sealed PII becomes permanently undecryptable while the chain still verifies. *(Driving worker loop awaits an erasure-request source; the handler exists and is tested.)*

**Integrity verification.** A worker loop signs a Merkle root over partition heads, anchors it to the external witness, and a standalone verifier recomputes the chain and compares (`AUD-2004` on divergence).

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `moderation` | upstream | Conformist + ACL | `moderation.v1.events` | a schema change to `decision_recorded` breaks the sealed DSA-rationale chain |
| `auth` | upstream | Conformist + ACL | `auth.v1.events` | session-lifecycle evidence gaps |
| `account` | upstream | Conformist + ACL | `account.v1.events` | PII sealing + the Art. 17 crypto-shred trigger break |
| `account` | dependency | Customer/Supplier | identity↔pseudonym mapping stays in `account` | audit cannot (and must not) resolve subjects |
| external witness / KMS | dependency | separate trust domain | RFC 3161 TSA / cross-account WORM; KMS SigV4 | the operator-level tamper guarantee weakens |

> **Anti-Corruption Layer:** `infrastructure/decode.rs` maps each foreign event wire shape (and the audit-owned `AuditEventWire` JSON) into the domain `AuditEvent` — upstream schema drift stops at this boundary.

---

## 8. Domain Events (semantics, not wire)

> Audit **publishes nothing of record** — it is a terminal sink. An integrity alarm on
> tamper/gap/witness-divergence is operational signal, not a System-of-Record stream.

| Event | Means | Emitted when | Who reacts |
|---|---|---|---|
| — (none) | audit asserts no business facts outward | — | — |

It **consumes** the compliance facts of `moderation`/`auth`/`account` — their meanings are owned by
those contexts' `DOMAIN.md §8` and rolled up in `docs/domain/EVENT_CATALOG.md`.

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| Audit is a separate tamper-evident evidence plane, not a log aggregator | [`ADR-0001`](../../../../docs/adr/0001-audit-is-a-separate-evidence-plane.md) | Accepted |
| _(candidate split-out)_ crypto-shred RtbF mechanism; dual-lane fail-open/fail-closed ingest; hash-chain + WORM + external-witness immutability | _to be written_ | — |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Supporting — invest enough to be legally bulletproof and tamper-evident; do not gold-plate beyond regulatory need.
- **Volatility:** low. The chain/shred/checkpoint model is stable; change is driven by new *regulatory* obligations (a new event category, a new lawful basis), not feature churn.
- **Known modeling debt:** the crypto-shred consumer and retention-expiry sweep handlers exist and are tested, but their driving worker loops await input sources (an erasure-request stream; resolved retention policies).
- **Deferred capabilities:** read-self-auditing (each authorized read as its own `DATA_ACCESS` event — read-authz itself is wired via the `auth-context` caller gate, deny-all when unconfigured); DSA transparency-report generation; cross-region ledger replication. KMS/witness *provisioning* is an IAM/org commitment (the code sits behind the ports).
