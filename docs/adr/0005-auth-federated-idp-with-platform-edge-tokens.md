# ADR-0005: Auth federates an IdP and issues platform ES256 edge tokens with generation-based revocation

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** auth; account; auth-context (verify lib); realtime; all services
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Authentication is three concerns that are easy to conflate: the **account** (who exists — a SoR),
**verification** (does this token check out — a per-service concern), and **issuance** (mint a
credential, manage the session). We also need fast, stateless-to-verify tokens at the edge *and*
the ability to revoke instantly — two goals that normally fight (stateless tokens can't be
recalled).

## Decision

`auth` is a distinct context that **federates an external IdP (Keycloak) for credentials** and
**issues the platform's own short-lived ES256 edge tokens**. It is separate from `account` (the
identity SoR) and from `auth-context` (the in-process verify library every service uses — a verify,
not a call). Instant revocation despite stateless verification is achieved with a **monotonic
per-subject `Generation`**: bumping it invalidates a whole token family; refresh tokens are
single-use (rotation invalidates the prior).

## Consequences

- **Positive:** verification is cheap and decentralized (`auth-context`, no call to auth per
  request); credentials reuse a hardened IdP; revocation is immediate via generation bump.
- **Negative / accepted trade-off:** the generation counter must be checked at verify time
  (a small lookup) for revocation to be timely; token lifetime tuning trades revocation latency
  against verification cost.
- **Closes:** the conflation of account/verify/issuance; the stateless-vs-revocable tension.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Opaque tokens verified by calling auth per request | Reintroduces a synchronous hop on every authenticated call |
| Long-lived tokens, no generation | No instant revocation; a leaked token stays valid until expiry |
| Build our own credential store instead of federating | Re-solves a generic, security-sensitive problem an IdP already solves |
