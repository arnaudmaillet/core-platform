<!--
================================================================================
 LIBRARY / INFRASTRUCTURE CRATE README — STANDARD TEMPLATE
================================================================================
 For crates under foundation/ · platform/ · storage/ — internal libraries
 consumed by the services, NOT deployable services themselves. For a deployable
 service, use SERVICE_README.template.md instead.

 Copy to crates/<tier>/<crate>/README.md and fill every <placeholder>. Delete
 these comments and any CONDITIONAL section that genuinely does not apply.

 PLACEHOLDER CONVENTION
   <crate-name>   directory name ............ e.g. scylla
   <package>      cargo package name ........ e.g. scylla-storage (if it differs)
   <Type>         a Rust type/trait ......... e.g. ScyllaSessionBuilder
   <CODE>         error-code prefix ......... e.g. SDB
   <...>          everything else

 WHAT MAKES THIS DIFFERENT FROM THE SERVICE TEMPLATE
   A library has no SLO, blast radius, on-call, events, or deployment. Its value
   is its CONTRACT (the public API) and its DECISIONS (why it is built this way,
   where the sharp edges are). So those sections lead, and "Key decisions" +
   "Gotchas" are first-class, not afterthoughts.

 SECTIONS
   CORE (always): Crate Card · Overview & role · Architecture & key decisions
     · Public API & contract · Integration · Configuration & feature flags
     · Testing · Gotchas / FAQ
   CONDITIONAL (include when the crate has one): Error model (has a code table)
     · Observability (emits spans/metrics) · Module layout (non-trivial tree)

 VOICE
   Contract + runbook for an engineer integrating or modifying the crate. Tables
   and precise signatures over prose. State the architectural boundary plainly:
   what this crate deliberately does NOT do.
================================================================================
-->

# `<crate-name>` — <one-line capability: the hard problem this crate abstracts for its consumers>

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `<foundation \| platform \| storage>` — `<one-line: what this tier is>` |
> | **Package** | `<package>` (dir: `crates/<tier>/<crate-name>`) |
> | **Consumed by** | `<services / crates that depend on it>` |
> | **Depends on** | `<key internal + external deps, e.g. scylla 1.5, telemetry, error>` |
> | **Stability** | `<stable contract \| evolving>` |
> | **Feature flags** | `<the cargo features that matter, or "none">` |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`<crate-name>` provides `<the capability / contract>` to the fleet. <2–4 sentences: what it
is, who uses it, and the one hard thing it gets right so its consumers don't have to.>

**Architectural boundary** — what this crate deliberately does **not** do: <e.g. "contains no
domain tables or CQL schema; all queries live in the service crates that depend on it">. Stating
the boundary is the point — it keeps the dependency graph honest.

---

## 📐 Architecture & key decisions

<Short structural description, then the decisions that a contributor MUST understand to avoid
breaking the crate. A library's README earns its keep here — capture the rationale, not just the
shape.>

```
<optional small diagram or data-flow if it clarifies the core mechanism>
```

- **<Decision 1>** — <what + why; the alternative rejected and the reason>.
- **<Decision 2>** — <…>.

---

## 🔌 Public API & contract

The surface consumers depend on (stable unless marked):

```rust
<the key types / traits / fns — real signatures, trimmed to the contract>
```

> **Contract notes:** <invariants callers must uphold; what is `pub` vs `pub(crate)` and why;
> anything that looks like API but isn't (sealed traits, `#[doc(hidden)]`, etc.)>.

---

## 📦 Integration

```toml
[dependencies]
<package> = { workspace = true }
```

```rust
<minimal end-to-end example: construct it, use the one method that matters>
```

---

## ⚙️ Configuration & feature flags

<CONDITIONAL env-var table — keep for storage/transport crates that read the environment;
drop for pure in-process libraries.>

| Variable | Default | Description |
|---|---|---|
| `<ENV_VAR>` | `<default>` | `<what it tunes>` |

**Feature flags:**
- `<feature>` — `<what it enables / why a consumer would turn it on>`.
- `build.rs`: `<what it generates, if anything (proto, descriptors)>`.

---

## 🧯 Error model &nbsp;·&nbsp; CONDITIONAL

<Include only if the crate defines an error type with stable codes.>

`<ErrorType>` implements `error::AppError`; codes map to gRPC `Status` / HTTP via the shared
`error` crate:

| Code | Variant | Retryable | Severity |
|---|---|---|---|
| `<CODE>-1001` | `<Variant>` | `<yes/no>` | `<…>` |

---

## 🔭 Observability &nbsp;·&nbsp; CONDITIONAL

<Include only if the crate emits spans/metrics (telemetry, transport, storage).>

```
<span hierarchy / key metrics>
```

---

## 🧪 Testing

```bash
cargo test   -p <package>
cargo clippy -p <package> --all-targets
```

<CONDITIONAL: integration tests needing infra — show the docker + env invocation.>

```bash
<docker run … ; ENV=… cargo test -p <package> -- --include-ignored>
```

---

## 🗂️ Module layout &nbsp;·&nbsp; CONDITIONAL

<Include for crates with a non-trivial tree; it is the fastest orientation a contributor gets.>

```
src/
├── <module>/      <one-line role>
└── <module>/      <one-line role>
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap a contributor or integrator will hit.

**1. `<symptom / surprising behavior>`.**
<Root cause + what to do. e.g. dependency-version quirks, sealed/private upstream types,
spawn/task-local pitfalls, feature-flag interactions.>

**2. `<…>`.**
<…>
