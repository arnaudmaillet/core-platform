# W8 v2 — Per-Service NetworkPolicy Call Graph

Basis for tightening the namespace-isolation baseline (#519) into per-service
micro-segmentation that isolates TIER-0 `audit`/`auth` from arbitrary peers.
Derived from code + config, not guesswork:

- **gRPC ports** — `k8s/base/services/*/.../{deployment,service}.yaml`.
- **gRPC mesh edges** — the tonic `*Client` types actually instantiated under
  `crates/services/*/src` (the *complete* set is 6 — see below) cross-checked with
  the `*_GRPC_ENDPOINT` config in `k8s/overlays/staging/*.env`.
- **Kafka event plane** — the generated topic-wiring in
  `docs/domain/EVENT_CATALOG.md` (authoritative: `crates/contracts/event-topology`).
- **Datastore egress** — the `*.env` per service.

> **Key result:** the intra-fleet gRPC mesh is **tiny** — only **6 services** receive
> calls from another fleet service. The rest is Kafka (egress to MSK, not pod→pod)
> or client-facing. That makes ingress micro-segmentation low-risk; egress lockdown
> is the harder half (managed AWS ENIs).

---

## 1. gRPC server ports

| Port | Service | Role |
|---|---|---|
| 50051 | chat | client-facing |
| 50052 | profile | **mesh callee** |
| 50053 | social-graph | **mesh callee** |
| 50054 | geo-discovery | client-facing |
| 50055 | notification | client-facing |
| 50056 | post | **mesh callee** |
| 50057 | comment | client-facing |
| 50058 | engagement | client-facing |
| 50059 | account | **mesh callee** |
| 50060 | auth | **mesh callee** (JWKS) |
| 50060 | timeline | client-facing (read) |
| 50061 | moderation | **mesh callee** (Screen) |
| 50062 | search | client-facing (read) |
| 50063 | media | client-facing |
| 50064 | counter-server | client-facing (read) |
| 50065 | counter-worker | worker (health only) |
| 50066 / 8443 | realtime-gateway | internal health / **public WSS** |
| 50067 | realtime-dispatcher | worker (health only) |
| 50068 | audit-server | TIER-0 (break-glass RecordPrivileged + Query) |
| 50069 | audit-worker | worker (health only) |

> ⚠️ **Side-finding — port collision:** `auth` and `timeline` both listen on **50060**.
> They're distinct ClusterIP services (different DNS/IPs), so k8s tolerates it and
> NetworkPolicy still works per-pod — but it's almost certainly a copy-paste slip
> (`timeline` should have its own port). Worth fixing independently of W8.

---

## 2. Inbound gRPC mesh (the complete set — 6 edges)

The only `*Client` types instantiated anywhere in `crates/services/*`:

| Caller | Client type | → Callee : port | Purpose |
|---|---|---|---|
| `auth` | `AccountServiceClient` | `account:50059` | account lookup during issuance |
| `moderation` | `AccountServiceClient` | `account:50059` | subject resolution |
| `counter` | `SocialGraphServiceClient` | `social-graph:50053` | follower/following reconcile |
| `timeline` | `SocialGraphServiceClient` / `SocialGraphGrpcClient` | `social-graph:50053` | fan-out + cold rebuild |
| `search` | `PostServiceClient` | `post:50056` | hydrate post docs |
| `search` | `ProfileServiceClient` | `profile:50052` | hydrate profile docs |
| `media` | `ModerationServiceClient` | `moderation:50061` | **fail-closed Screen gate** |
| `realtime` | `JwksClient` | `auth:50060` | fetch JWKS to verify edge tokens |

### Inbound matrix (who a policy must allow)

| Callee | Allowed in-mesh callers | Port |
|---|---|---|
| `account` | `auth`, `moderation` | 50059 |
| `social-graph` | `counter`, `timeline` | 50053 |
| `post` | `search` | 50056 |
| `profile` | `search` | 50052 |
| `moderation` | `media` | 50061 |
| `auth` | `realtime` | 50060 |

**No in-mesh inbound at all** (→ ingress = health probe only, + the client entry
point if/when one exists): `chat`, `geo-discovery`, `notification`, `comment`,
`engagement`, `timeline`, `search`, `media`, `counter-server`, and the workers
`counter-worker`, `realtime-dispatcher`, `audit-worker`. `audit-server` takes only
the break-glass `RecordPrivileged`/`Query` path (no normal-flow mesh caller).
`realtime-gateway` takes public WSS on 8443 (already allowed in #519).

---

## 3. ⚠️ Open decision — the client-facing entry point

The "client-facing" services above are read/command APIs meant to be called by a
gateway/BFF, **not** by other fleet services. But:

- there is **no in-cluster BFF/gateway** in this repo or the staging overlay (only a
  `gateway-bff-deploy.yml` workflow — the BFF lives off-fleet);
- staging has **no ALB ingress** (only the realtime NLB).

So today those services have **no in-cluster inbound** beyond health probes — they
could be locked down hard right now. But that would block the BFF/edge the moment
it's introduced. **Decision needed before writing their ingress policies:**

1. **Where does client traffic enter?** An in-cluster BFF pod (→ allow by its label),
   an ALB (→ allow from the ALB/VPC ipBlock), or stays external-only for now (→ lock
   to health probes + revisit when the BFF lands)?

Until that's answered, the safe move is to keep the **same-namespace baseline**
(#519) for the client-facing set and only tighten the **6 mesh callees** + the
**TIER-0/worker** services, whose inbound is fully known.

---

## 4. Kafka event plane (egress to MSK, not pod→pod)

From `EVENT_CATALOG.md` — these are producer→consumer over MSK, so they are
**egress to the broker ENIs**, never pod-to-pod ingress. They do NOT need ingress
allows; they inform the **egress** policy (who needs MSK :9096).

| Topic | Producer | Consumers |
|---|---|---|
| `account.v1.events` | account | audit, profile |
| `profile.v1.events` | profile | search, post |
| `post.v1.events` | post | timeline, search, realtime |
| `post.published` / `post.deleted` | post | geo-discovery, notification / timeline |
| `comment.created` / `comment.deleted` | comment | notification, engagement / engagement |
| `engagement.reactions` | engagement | counter, notification, engagement |
| `social-graph.*` (followed/unfollowed/tier) | social-graph | timeline, profile |
| `counter.v1.popularity` | counter | realtime, geo-discovery |
| `moderation.v1.events` | moderation | audit, search, media |
| `auth.v1.events` | auth | audit |
| `media.v1.events` | media | media |
| `chat.*` | chat | chat (rest orphan) |

**MSK producers/consumers** (need egress :9096): account, profile, post, comment,
engagement, social-graph, counter (+worker), moderation, auth, media, chat,
geo-discovery, notification, timeline, search, realtime (+dispatcher), audit (+worker).
(≈ everyone except the pure read paths.)

---

## 5. Egress map (per service) — for the v2 egress lockdown

Every pod also needs: **DNS** → `kube-system` CoreDNS :53 (UDP/TCP), and **OTel** →
`otel-collector.observability.svc` :4317.

| Service | Datastores / object store (egress) | gRPC callees | Kafka |
|---|---|---|---|
| account | CNPG `account` | — | producer |
| auth | CNPG `auth`, Redis | account:50059 | producer |
| profile | CNPG, Redis, Scylla | — | both |
| social-graph | CNPG, Redis, Scylla | — | both |
| post | CNPG, Scylla | — | both |
| comment | CNPG, Scylla | — | both |
| engagement | CNPG, Redis, Scylla | — | both |
| counter (server+worker) | CNPG, Redis, Scylla | social-graph:50053 | both |
| geo-discovery | CNPG, Redis, Scylla | — | consumer |
| notification | CNPG, Redis, Scylla | — | consumer |
| timeline | CNPG, Redis, Scylla | social-graph:50053 | consumer |
| chat | CNPG, Redis, Scylla | — | producer |
| moderation | CNPG, Redis, Scylla | account:50059 | both |
| media (server) | CNPG, Redis, **S3** (asset + object-store) | moderation:50061 | both |
| search | **OpenSearch** | post:50056, profile:50052 | consumer |
| audit (server+worker) | CNPG, **S3** (WORM/witness), KMS | — | consumer |
| realtime (gateway+dispatcher) | Redis | auth:50060 (JWKS) | consumer |

**Managed-AWS egress targets** (no pod IP — use ipBlock of the **private-data subnet
CIDRs**): MSK :9096, ElastiCache :6379, OpenSearch :443. **S3** → via the S3 gateway
endpoint (NetworkPolicy can't name a prefix list; allow :443 to the VPC/`0.0.0.0/0`
or rely on the gateway-endpoint route). **Scylla** → `scylla` ns :9042 (namespace
selector). **CNPG** → same-ns :5432 (pod selector `cnpg.io/cluster`).

---

## 6. Proposed policy shape (v2)

Layered on top of the #519 baseline:

**Ingress (do now — fully known):**
- Per mesh callee (`account`, `social-graph`, `post`, `profile`, `moderation`,
  `auth`): replace the broad same-ns allow with an allow **only** from the specific
  caller pods on the specific port (table §2).
- Workers (`counter-worker`, `realtime-dispatcher`, `audit-worker`) and `audit-server`:
  deny all mesh ingress (health probes are node→pod, permitted by the VPC CNI).
- `realtime-gateway`: keep the public-WSS allow (#519); 50066 health only.
- Client-facing set: **hold at same-ns** until §3 is decided.

**Egress (do after ingress proves stable — riskier):**
- Namespace-wide allow: DNS (kube-system :53), OTel (observability :4317).
- Per service: its datastores (Scylla ns / same-ns CNPG / data-subnet ipBlock for
  ElastiCache+OpenSearch+MSK / S3 :443) + its gRPC callee(s) from §5.
- Flip on `default-deny-egress` **last**, per service, watching for drops.

---

## 7. Open decisions before implementing

1. **Client entry point** (§3) — BFF pod label, ALB ipBlock, or lock-to-probes-for-now?
2. **Egress scope** — do we want full egress lockdown now, or ingress-only v2 first?
   (Egress needs the live data-subnet CIDRs + S3 handling; higher breakage risk.)
3. **Port collision** — fix `auth`/`timeline` both on 50060 first (independent of W8).
4. **CNI** — confirm `enableNetworkPolicy=true` (shipped in #519) is live before any
   of this enforces.

Once 1–2 are answered, the per-service policies are mechanical to generate from the
matrices above. Rollout slots into Phase 3c of
`docs/runbooks/audit-remediation-rollout.md` (apply allows first, deny last, watch).
