# `validate-core` — Domain & Functional Contract

> The zero-dependency validation abstraction: a Separated Interface answering *"how can `cqrs` and `validation` share a validation contract without depending on each other?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | The validation abstraction boundary — the `Validate` trait + `FieldViolation` primitive |
> | **Layer** | `foundation` — a true graph-leaf; **zero dependencies** by hard constraint |
> | **Subdomain class** | **Generic** — a Separated Interface; its entire value is the dependency-inversion it enables |
> | **Primary abstraction(s)** | `Validate` + `FieldViolation` (`validate_core`) |
> | **Footprint** | pure (zero deps, no IO, no state) — the empty feature set *enforces* zero-dep |
> | **Failure posture** | N/A — it only *describes* field-level failures |
> | **Depends on** | **nothing** (enforced) |
> | **Consumed by** | `cqrs` (as a `Command` supertrait), `validation` (middleware + error type) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `validate-core` is the fleet's authority for the **validation abstraction**: it answers
**"what does it mean for a type to validate itself, and how do `cqrs` and `validation` agree on that without
coupling to each other?"**

**The hard problem.** `cqrs::Command` needs `Validate` as a supertrait, and `validation` provides the
middleware + concrete error type — but if `Validate` lived in `validation`, then `cqrs` would inherit
`validation`'s entire middleware/HTTP/error stack. A third, dependency-free crate lets both sides point
*inward* at the same abstraction. The zero-dependency constraint is not incidental — it is the crate's
entire reason to exist.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Provide the validation *middleware* or the `AppError`/HTTP mapping → owned by `validation`.
- ❌ Pull in any framework, async runtime, or `std::collections` → that would defeat the inversion.
- ❌ Decide *when* validation runs in the pipeline → that is `validation`'s composition decision.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Field violation | A single field-level failure (field path + stable code + message) | `FieldViolation` |
| Validation code | A stable, machine-readable `VAL-xxxx` code | `FieldViolation::code` (`&'static str`) |
| Validate | The trait a type implements to express its own invariant checks | `Validate` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `FieldViolation` | value type | `field`/`code` are `&'static str` (no heap on the hot path); `message` is the only allocation |
| `Validate` | trait (seam) | `validate()` must **aggregate** all violations, never short-circuit; default is a no-op `Ok(())` |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- Exactly two public items — `FieldViolation` and `Validate`. Nothing else, by design.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| `ValidationLayer` middleware + `ValidationError` | `validation` | The operational half depends *up* on this abstraction |
| The `Command` supertrait wiring | `cqrs` | `cqrs` depends *up* on this abstraction, not on `validation` |

**The "do-not-depend-on" list:** **everything.** `[dependencies]` must remain empty; no `use` of
`std::collections`, async, or any error framework. CI/review pushes back on any added edge — the zero-dep
guarantee is the contract.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | The crate has **zero** dependencies | empty `[dependencies]` + empty feature set | CI/review rejection |
| I2 | `validate()` aggregates *all* violations (no short-circuit) | contract convention | partial error reporting |
| I3 | A returned `Err(violations)` is non-empty | convention (`validation::ValidationError::new` debug-asserts it) | debug panic downstream |
| I4 | `field`/`code` stay `&'static str` (never `String`) | type definition | heap on the validation path |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

N/A — pure abstraction crate, no runtime control flow. A type's `validate()` runs synchronously where the
caller invokes it (in practice, inside `validation::ValidationLayer`, before the first `.await`). There is no
state, no lifecycle, no background work.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `cqrs` | downstream | Separated Interface | `Validate` as a `Command` supertrait | every command's validate-ability |
| `validation` | downstream | Separated Interface | `ValidationError` wraps `Vec<FieldViolation>` | the middleware + error mapping |

> **Stability seam:** `Validate` and `FieldViolation` are the entire public API; a change ripples to both
> consumers at once. The convergence shape (both point inward, neither at the other) is the architectural
> guarantee.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

N/A — pure, zero-dependency. It emits nothing (no `tracing` even — that would be a dependency). The
"validation failed" log is emitted by `validation`.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Separated Interface in a third crate (not a module in `validation`) to break the `cqrs`↔`validation` cycle | [`README §Architecture`](../README.md) | Accepted |
| Aggregation over short-circuit (all failing fields in one round-trip) | [`README §Architecture`](../README.md) | Accepted |
| Zero dependencies as an enforced, load-bearing constraint | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — a Separated Interface; its leverage is purely the dependency graph it makes
  possible.
- **Stability:** stable contract — the two items have settled.
- **Volatility:** minimal — any growth risks the zero-dep guarantee, so growth is actively resisted.
- **Deferred capabilities:** none; richer validation (async, cross-field) would live in `validation`, never
  here.
