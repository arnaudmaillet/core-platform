# ADR-0001: Audit is a separate tamper-evident evidence plane, not a log aggregator

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** audit (new TIER-0 context); producers moderation, auth, account
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

We need an authoritative record of **who did what to whom, when, and under what authority** — for
SOC2, the DSA, and GDPR accountability (Art. 5(2)). The instinct is to derive this from telemetry
(logs/traces). That instinct is wrong, and provably so: **telemetry is mutable, sampled, and
retention-capped, whereas evidence must be tamper-evident, complete, and non-repudiable.**

Worse, conflating the two manufactures a direct legal contradiction. GDPR Art. 17 (right to
erasure) demands we can delete a subject's data; Art. 5(2) (accountability) demands we can prove
what happened to it. If the proof *is* the data, satisfying one violates the other. A log
aggregator or per-service audit tables cannot resolve this — they have no global chain, and
deleting a row to honour erasure silently destroys the accountability record.

## Decision

We build **`audit` as a dedicated TIER-0 append-only, hash-chained evidence plane** — a terminal
sink that *records decisions and never acts*, distinct from the telemetry plane. Constituent rules:

1. **Immutability = four independent trust domains to forge.** A 3-layer hash-chain over events,
   stored INSERT-only in Postgres **and** mirrored to Object-Lock WORM, with a signed Merkle
   checkpoint anchored to an external witness. Forging history requires compromising all four.
2. **GDPR RtbF via crypto-shred, not deletion.** Each subject's PII is sealed under a per-subject
   DEK with a pseudonym; the chain hashes the *ciphertext*. Erasure = destroy the key — the record
   and its proof survive, the PII becomes unrecoverable. Legal-hold overrides erasure.
3. **Dual-lane ingest.** Async Kafka (`run_consumer`, **fail-open** producer) carries ~99% of
   volume and absorbs spikes; a synchronous gRPC `RecordPrivileged` lane is **fail-closed** — a
   break-glass action is *denied* if it cannot be recorded first.
4. **Hybrid partitioning** by tenant + category, with subject indexed for erasure lookups.
5. Realised as two binaries: `audit-server` (`:50068`) and `audit-worker` (`:50069`); error
   namespace `AUD-XXXX`.

## Consequences

- **Positive:** erasure (Art. 17) and accountability (Art. 5(2)) coexist without contradiction;
  tampering requires breaching four trust domains; a break-glass action can never proceed
  unrecorded; SOC2/DSA evidence requirements are met by construction.
- **Negative / accepted trade-off:** a new TIER-0 service to operate; the synchronous lane adds a
  hard dependency on the break-glass path; production key custody (KMS/HSM) and the external
  witness (RFC3161 / cross-account WORM) are deferred to IAM/org provisioning; crypto-shred and
  retention-sweep consumers await upstream sources.
- **Closes:** the GDPR Art. 17 ⇄ Art. 5(2) contradiction, and the SOC2/DSA evidentiary gap.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Derive audit from logs / a SIEM | Mutable, sampled, retention-capped — cannot prove non-repudiation or completeness |
| Per-service audit tables | No global chain; honouring erasure by deleting rows destroys the accountability record |
| Store PII plaintext, delete on request | Deletion breaks the hash-chain; erasure and proof become mutually exclusive |
