# ADR-0011: Media is a byte-free control plane with fail-open delivery and a fail-closed CSAM Screen

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** media; moderation; post, profile, search
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Media means large binaries. Pushing bytes through gRPC or Kafka wrecks both (message-size limits,
broker pressure, memory). Yet the asset *lifecycle* — uploaded → screened → transformed → ready —
must be tracked authoritatively, must **never** let unsafe content (CSAM) reach `ready`, and must
keep serving even when a non-safety dependency is degraded.

## Decision

`media` is a **byte-free control plane**: bytes go **direct to object storage via pre-signed URLs**,
never over gRPC/Kafka; media owns the asset **metadata/lifecycle SoR** (Postgres + Redis), with the
bytes in S3/MinIO referenced by `StorageKey`. It runs three planes — (A) upload broker, (B) async
Kafka transform pipeline, (C) delivery/CDN brokerage — with a **per-category failure posture**:
delivery resolution **fails open**, while the **CSAM `Screen` gating readiness fails closed** (an
asset cannot reach `ready` without passing it; a moderation takedown quarantines/deletes).

## Consequences

- **Positive:** the byte plane never stresses the mesh; the lifecycle is authoritative; unsafe
  content can't go live; delivery degrades gracefully.
- **Negative / accepted trade-off:** clients must handle the pre-signed-upload handshake; the Screen
  is a synchronous dependency on readiness (bounded by a hard timeout).
- **Closes:** bytes-on-the-wire pressure and the unsafe-content-goes-live risk.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Stream bytes through the service / Kafka | Size limits, broker pressure, memory blowup |
| Store bytes in the database | Wrong tool; bloats the SoR; no CDN path |
| Fail-open Screen | Lets CSAM reach `ready` during an outage — unacceptable |
