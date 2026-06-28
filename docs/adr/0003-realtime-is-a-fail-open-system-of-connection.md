# ADR-0003: Realtime is a fail-open System-of-Connection, never a record store

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** realtime (new TIER-1 context); chat, notification, counter, post
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Live client push (DMs, notifications, engagement spikes) is already implemented twice — `chat` and
`notification` each stream to clients in their own ad-hoc way. A third bespoke real-time path would
compound the duplication and the operational surface. We need **one delivery plane**. Two traps to
avoid: polling (an inverted load-amplifier — clients hammer the mesh to discover nothing changed),
and letting the delivery plane accrete durability it should not own (at which point an outage loses
data instead of being recoverable).

## Decision

We build **`realtime` as a TIER-1, fail-open System-of-Connection / Delivery** — a bulkhead in
front of the gRPC mesh that **never stores records**: if it vanishes, clients re-sync from the
owning SoRs. Constituent rules:

1. **One multiplexed socket per device.** WSS on `:443` carrying an envelope
   `{stream_seq, channel, ack_required, payload}` — **not** gRPC-to-client, **not** SSE. One socket
   replaces the per-feature client streams.
2. **Targeted fan-out.** Registry lookup + targeted delivery, not broadcast-and-filter.
3. **Fail-open with delegated durability.** If a recipient is offline, delegate to
   `notification` (APNs/FCM); on reconnect the client re-syncs from the SoRs via a sequence token.
   Lost in-flight delivery is never lost data.
4. **Split listeners.** Internal gRPC on `:50066` (gateway) and `:50067` (dispatcher); the public
   WebSocket is a *separate* listener — deliberately breaking the fleet one-port convention. Two
   binaries: `realtime-gateway` (stateful edge) and `realtime-dispatcher` (stateless worker).

## Consequences

- **Positive:** one multiplexed socket per device collapses the chat/notification silos; failure
  cannot lose data because realtime owns none — recovery is a re-sync; connection load is isolated
  from the gRPC mesh.
- **Negative / accepted trade-off:** a stateful edge with C10M concerns (parked-future idle conns,
  hard per-connection memory SLO, bounded shed queue, heartbeat reaping); it intentionally breaks
  the one-port convention, which the deployment must accommodate.
- **Closes:** the third ad-hoc real-time silo, and the polling load-amplification path.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Client polling | Inverted load-amplifier — most polls discover nothing, hammering the mesh |
| gRPC-streaming-to-client / SSE | No multiplexing and no client-side envelope; would re-create per-feature streams |
| Broadcast-and-filter fan-out | Wasteful at scale; every node processes every message to discard most |
| Make realtime a record store | Forces durability guarantees it should not own; defeats the fail-open recovery model |
