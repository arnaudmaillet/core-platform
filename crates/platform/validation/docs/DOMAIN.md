# `validation` — Domain & Functional Contract

> The operational half of validation: middleware + error type. It answers *"where in the command pipeline do invalid inputs get rejected, and how is that failure shaped for clients?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Input-validation middleware for the CQRS command bus + the concrete `ValidationError` (422) and the `VAL-xxxx` catalogue |
> | **Layer** | `platform` — the operational half of the `validate-core` abstraction |
> | **Subdomain class** | **Generic** — standard input validation; leverage is early rejection + aggregated field errors |
> | **Primary abstraction(s)** | `ValidationLayer` + `ValidationError` (`validation`) |
> | **Footprint** | pure (in-process middleware, no IO, no env) |
> | **Failure posture** | **fail-closed for invalid input** — rejected before any span, idempotency record, or DB work |
> | **Depends on** | `validate-core`, `cqrs`, `error` |
> | **Consumed by** | service composition roots (placed outermost on the command pipeline) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `validation` is the fleet's authority for **input-validation enforcement**: it answers
**"how is the abstract `Validate` contract turned into pipeline middleware that rejects invalid commands at the
earliest point, with a client-shaped 422 carrying every failing field?"**

**The hard problem.** Validation must reject *before* any async resource is consumed (span, idempotency record,
DB transaction), aggregate all field errors into one round-trip, and cost nothing for valid commands — while
keeping the abstract `Validate` trait in a dependency-free crate so `cqrs` can require it without inheriting this
middleware. `validation` is the operational half; `validate-core` is the abstraction.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Define the `Validate` trait or `FieldViolation` → those live in `validate-core` (so `cqrs` depends on the
  abstraction, not this middleware).
- ❌ Decide a command's *rules* → each command's `Validate` impl owns those.
- ❌ Own pipeline placement as config → it is a composition-root decision (outermost).

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Validation layer | The zero-size `CommandLayer` that calls `validate()` before dispatch | `ValidationLayer`, `ValidationCommandBus` |
| Validation error | The concrete `AppError` (422, `Severity::Low`) wrapping the violations | `ValidationError` |
| Details map | `field → "VAL-xxxx: message"` for `ApiErrorResponse.details` | `ValidationError::to_details_map` |
| VAL-xxxx catalogue | The stable validation-code constants | `VAL_1001_REQUIRED` … `VAL_9000_CUSTOM` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `ValidationLayer` | zero-size `CommandLayer` | Must be the **outermost** layer; inlines `validate()`, eliminated for no-op commands |
| `ValidationCommandBus<S>` | decorated bus | Calls `payload.validate()`; `Err` short-circuits before the handler runs |
| `ValidationError` | `AppError` impl | `error_code "VAL-0001"`, HTTP 422, `Severity::Low`, retryable false, category `VALIDATION` |
| `to_details_map()` | method | Aggregated `field → code: message`, straight into `ApiErrorResponse.details` |
| `VAL_xxxx` constants | catalogue | Stable codes (`1001 REQUIRED`…`9000 CUSTOM`) |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The middleware (`ValidationLayer`/`ValidationCommandBus`), the concrete `ValidationError`, and the `VAL-xxxx`
  constant catalogue.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| The `Validate` trait + `FieldViolation` | `validate-core` | So `cqrs` depends on the abstraction without this middleware |
| Per-command validation rules | each command's `Validate` impl | The middleware is generic over `C: Command` |
| The `AppError` contract / wire shape | `error` | `ValidationError` *implements* it |

**The "do-not-depend-on" list:** never a service/domain crate; it sits between `cqrs` (extends it) and
`validate-core`/`error` (conforms to them).

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `ValidationLayer` is the **outermost** command layer | composition root (`.layer()` first) | wasted work before rejection |
| I2 | Rejection happens before the first `.await` (no async resource consumed) | `ValidationCommandBus` | invalid commands cost DB/span/idempotency work |
| I3 | All failing fields returned in one round-trip | `to_details_map` over `Vec<FieldViolation>` | piecemeal client errors |
| I4 | Valid commands incur ~zero cost (zero-size layer, inlined `validate()`) | type system | unnecessary overhead |
| I5 | `CqrsError::Handler` wraps a type-erased error — not downcastable to `ValidationError` | `cqrs` erasure | inspect via `error_code()`/`Display`, not downcast |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Per command.** `ValidationCommandBus` (outermost) calls `envelope.payload.validate()`:
- `Ok(())` → forward to the inner pipeline (idempotency → tracing → logging → handler).
- `Err(violations)` → wrap as `ValidationError` → return `CqrsError::Handler(ValidationError)`; the handler is
  **never** called and no async resource is touched.

For commands using the no-op `Validate` default, the compiler eliminates the call entirely — valid commands pay
nothing.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `validate-core` | upstream | Separated Interface | `Validate` + `FieldViolation` | what the middleware can validate |
| `cqrs` | upstream | Open-Host (extends) | `ValidationLayer: CommandLayer` | pipeline integration |
| `error` | upstream | Conformist | `ValidationError: AppError` (422) | client error mapping |
| service composition roots | downstream | Published Contract | place `ValidationLayer` outermost | input rejection |

> **Stability seam:** `ValidationError` (`error_code "VAL-0001"`) and the `VAL-xxxx` catalogue are public,
> client-visible API; `ValidationLayer`'s outermost-placement rule is a contract.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| `command validation failed — dispatch short-circuited` | `tracing` DEBUG (`command.type`, `violation.count`) | a command fails validation | dashboards filtered on `category = VALIDATION` |

DEBUG is intentional — validation failures are expected client behaviour, not incidents. No external state
mutation. A spike on `error_code = VAL-0001` may indicate a client breaking change or a bad deploy.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Outermost placement — reject before idempotency/tracing/DB | [`README §Architecture`](../README.md) | Accepted |
| Zero-cost for valid commands (zero-size layer, inlined `validate()`) | [`README §Architecture`](../README.md) | Accepted |
| Aggregated field errors via `to_details_map()` | [`README §Architecture`](../README.md) | Accepted |
| Trait/middleware split with `validate-core` (breaks the `cqrs`↔`validation` cycle) | [`validate-core README`](../../../foundation/validate-core/README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — standard input validation; leverage is early rejection at scale + aggregated
  field errors.
- **Stability:** stable contract — `VAL-0001` and the `VAL-xxxx` catalogue are client-visible.
- **Volatility:** low — new codes are additive constants.
- **Deferred capabilities:** structured downcast access to `ValidationError` after `cqrs` erasure (today:
  inspect via `error_code()`/`Display`, or validate explicitly before dispatch).
