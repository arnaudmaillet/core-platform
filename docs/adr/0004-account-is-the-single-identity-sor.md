# ADR-0004: Account is the single identity SoR and owns an outbound event lane

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** account; consumers audit, profile
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

A real identity (email, phone, KYC, roles) must live in exactly one place, or GDPR erasure becomes
impossible — every shadow copy is an uncontrolled record that Art. 17 can't reach. Account was also
a **phantom producer**: it owned identity but published nothing, so downstream contexts had no way
to react to lifecycle changes except by reaching into account's store.

## Decision

Account **is the single System of Record for the user account** — existence, credential metadata,
KYC, roles, and GDPR state — and **owns an outbound event lane** (`account.v1.events`, published
after each durable save). Downstream contexts (`audit`, `profile`) consume references; none holds
authoritative identity. A `gdpr_deletion_requested` event propagates erasure (audit crypto-shreds
the subject), closing the Art. 17 loop end to end.

## Consequences

- **Positive:** identity has one home; GDPR erasure has a single authoritative trigger that
  propagates; downstream stays decoupled (events, not store reach-in).
- **Negative / accepted trade-off:** account must guarantee event emission after save (an outbox/
  after-save publish discipline), adding write-path responsibility.
- **Closes:** the phantom-producer gap and the shadow-identity erasure problem.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Let services read account's store directly | Couples every consumer to account's schema; no erasure propagation |
| Replicate identity into each service | Multiplies PII copies; makes Art. 17 erasure unenforceable |
