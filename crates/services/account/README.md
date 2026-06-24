# `account` — Core Account Microservice

Manages the complete private lifecycle of a physical person on the platform:
identity verification, credentials, KYC compliance, GDPR rights, and
role-based access control. Financial state is owned by the dedicated
`services/ledger` microservice (Single Responsibility at hyperscale).

---

## Architecture

```
crates/services/account/
├── proto/account/v1/          # Protobuf contracts
│   ├── enums.proto            # AccountStatus, KycStatus, AccountRole
│   ├── messages.proto         # Request / response / view messages
│   └── service.proto          # AccountService RPC definitions
├── migrations/                # SQL migrations (CockroachDB-compatible)
├── src/
│   ├── domain/
│   │   ├── aggregate/         # Account — DDD aggregate root
│   │   ├── entity/            # MfaState, GdprRecord
│   │   ├── event/             # Domain events (AccountCreated, …)
│   │   └── value_object/      # AccountId, EmailAddress, PasswordHash, …
│   ├── application/
│   │   ├── port/              # AccountRepository trait (hex port)
│   │   ├── command/           # 17 command handlers (CQRS)
│   │   └── query/             # 5 query handlers (CQRS)
│   ├── infrastructure/
│   │   ├── persistence/       # Postgres adapter (CockroachDB-compatible SQL)
│   │   └── grpc/              # Tonic gRPC server + handler
│   ├── error.rs               # AccountError enum
│   └── lib.rs
└── Cargo.toml
```

The service follows **Clean Architecture**: the `domain/` layer is completely
free of I/O dependencies; the `application/` layer contains pure CQRS handlers;
all I/O lives in `infrastructure/`.

---

## gRPC API — `account.v1.AccountService`

Proto package: `account.v1`  
Proto source: `proto/account/v1/service.proto`

### Commands (mutating RPCs)

All commands return `CommandResponse { success: bool, account_id: string }`.

| RPC | Request | Description |
|-----|---------|-------------|
| `CreateAccount` | `CreateAccountRequest` | Register a new account. Idempotent on `(identity_id, email)`. |
| `VerifyEmail` | `VerifyEmailRequest` | Mark the email address as verified. |
| `VerifyPhone` | `VerifyPhoneRequest` | Mark the phone number as verified. |
| `ChangePassword` | `ChangePasswordRequest` | Replace the stored Argon2id password hash. |
| `EnrollMfa` | `EnrollMfaRequest` | Store an AES-256-GCM encrypted TOTP seed and activate MFA. |
| `RevokeMfa` | `RevokeMfaRequest` | Remove all MFA credentials. |
| `UpdateKycStatus` | `UpdateKycStatusRequest` | Update the KYC verification outcome (admin/compliance only). |
| `SuspendAccount` | `SuspendAccountRequest` | Suspend an account (admin action). |
| `ReactivateAccount` | `ReactivateAccountRequest` | Lift a suspension and return to Active. |
| `DeactivateAccount` | `DeactivateAccountRequest` | User-initiated self-deactivation. |
| `RecordLogin` | `RecordLoginRequest` | Record a successful login; reset failed-attempt counter. |
| `RecordFailedLogin` | `RecordFailedLoginRequest` | Increment failed-attempt counter; lock if threshold reached. |
| `RequestGdprDeletion` | `RequestGdprDeletionRequest` | Initiate Art. 17 right-to-erasure request. |
| `AnonymizeAccount` | `AnonymizeAccountRequest` | Anonymise all PII after retention period has elapsed. |
| `RequestDataExport` | `RequestDataExportRequest` | Initiate Art. 20 data portability export. |
| `AssignRole` | `AssignRoleRequest` | Grant an internal platform role (admin only). |
| `RevokeRole` | `RevokeRoleRequest` | Remove an internal platform role (admin only). |

### Queries (read RPCs)

| RPC | Request | Response | Description |
|-----|---------|----------|-------------|
| `GetAccountById` | `GetAccountByIdRequest` | `AccountView` | Full aggregate view by primary key (UUIDv7). |
| `GetAccountByIdentityId` | `GetAccountByIdentityIdRequest` | `AccountView` | Full aggregate view by IdP subject claim. |
| `GetAccountStatus` | `GetAccountStatusRequest` | `AccountStatusView` | Lightweight status used by auth middleware and gateway. |
| `GetGdprRecord` | `GetGdprRecordRequest` | `GdprRecordView` | Full GDPR compliance record (restricted endpoint). |
| `ListAccountsByStatus` | `ListAccountsByStatusRequest` | `ListAccountsByStatusResponse` | Paginated account list filtered by lifecycle status. |

---

## Domain Model

### `Account` (aggregate root)

| Field | Type | Notes |
|-------|------|-------|
| `id` | `AccountId` (UUIDv7) | Shard key for distributed routing |
| `identity_id` | `IdentityId` | External IdP subject claim |
| `email` | `EmailAddress` | Unique; RFC 5321-normalised |
| `email_verified` | `bool` | Set via `VerifyEmail` |
| `phone` | `Option<PhoneNumber>` | E.164 format |
| `phone_verified` | `bool` | Set via `VerifyPhone` |
| `password_hash` | `Option<PasswordHash>` | Argon2id; `None` for SSO-only accounts |
| `roles` | `Vec<AccountRole>` | Platform roles (additive) |
| `status` | `AccountStatus` | Lifecycle state machine |
| `kyc_status` | `KycStatus` | Verification state machine |
| `mfa` | `MfaState` | Embedded MFA state |
| `gdpr` | `GdprRecord` | Embedded GDPR compliance record |
| `version` | `i64` | Optimistic-lock version counter |
| `created_at` | `DateTime<Utc>` | Immutable after creation |
| `updated_at` | `DateTime<Utc>` | Updated on every write |

### Status State Machines

**AccountStatus transitions:**

```
PendingVerification ──▶ Active ──▶ Suspended ──▶ Active
                  ╰──────────────▶ Deactivated
                  ╰──────────────▶ Deleted
Active            ──▶ Deactivated
Active            ──▶ Deleted
Deactivated       ──▶ Deleted
```

**KycStatus transitions:**

```
NotStarted ──▶ Submitted ──▶ InReview ──▶ Approved
                                      ╰──▶ Rejected ──▶ Submitted (resubmission)
```

### Enums

**AccountStatus**

| Proto value | Rust variant | i32 |
|-------------|-------------|-----|
| `ACCOUNT_STATUS_PENDING_VERIFICATION` | `PendingVerification` | 1 |
| `ACCOUNT_STATUS_ACTIVE` | `Active` | 2 |
| `ACCOUNT_STATUS_SUSPENDED` | `Suspended` | 3 |
| `ACCOUNT_STATUS_DEACTIVATED` | `Deactivated` | 4 |
| `ACCOUNT_STATUS_DELETED` | `Deleted` | 5 |

**KycStatus**

| Proto value | Rust variant | i32 |
|-------------|-------------|-----|
| `KYC_STATUS_NOT_STARTED` | `NotStarted` | 1 |
| `KYC_STATUS_SUBMITTED` | `Submitted` | 2 |
| `KYC_STATUS_IN_REVIEW` | `InReview` | 3 |
| `KYC_STATUS_APPROVED` | `Approved` | 4 |
| `KYC_STATUS_REJECTED` | `Rejected` | 5 |

**AccountRole**

| Proto value | Rust variant | i32 |
|-------------|-------------|-----|
| `ACCOUNT_ROLE_USER` | `User` | 1 |
| `ACCOUNT_ROLE_CONTENT_MODERATOR` | `ContentModerator` | 2 |
| `ACCOUNT_ROLE_SUPPORT_AGENT` | `SupportAgent` | 3 |
| `ACCOUNT_ROLE_FINANCE_OPERATOR` | `FinanceOperator` | 4 |
| `ACCOUNT_ROLE_ADMIN` | `Admin` | 5 |
| `ACCOUNT_ROLE_SUPER_ADMIN` | `SuperAdmin` | 6 |

---

## Persistence

### Database

CockroachDB / distributed PostgreSQL. All SQL uses **ANSI semantics** — no
PostgreSQL-specific extensions. The primary key is a UUIDv7, which provides
time-sortable, append-friendly clustering compatible with CockroachDB's range
partitioning.

### Optimistic Locking

All writes use a compare-and-swap pattern:

```sql
UPDATE accounts SET ..., version = version + 1
WHERE id = $1 AND version = $37
```

Zero rows affected → `AccountError::ConcurrentModification` (retryable).

### Sharding

`AccountId` implements `ShardKey` by feeding its UUID bytes into the hasher.
All repository writes use `run_on_shard(&account_id, |tx| ...)` for
topology-agnostic transaction routing.

---

## Security Constraints

| Concern | Implementation |
|---------|----------------|
| Password storage | Argon2id hash only; plaintext never accepted or stored |
| TOTP seed storage | AES-256-GCM encrypted (`EncryptedBytes`); key managed externally |
| Log redaction | `PasswordHash` and `EncryptedBytes` suppress `Display` and `Debug` outputs |
| Recovery codes | Bcrypt-hashed `RecoveryCodeHash` values; plaintext never persisted |
| Secret fields | `#[serde(skip)]` on `PasswordHash` and `EncryptedBytes` in aggregate |

---

## GDPR Compliance

| Right | Implementation |
|-------|----------------|
| Art. 17 — Erasure | `RequestGdprDeletion` records request + scheduled deletion date; `AnonymizeAccount` overwrites PII fields |
| Art. 20 — Portability | `RequestDataExport` records request; `data_export_completed_at` tracks fulfilment |
| Consent audit | `data_processing_consented_at`, `marketing_consented_at`, `consent_ip`, `last_consent_version` stored in `GdprRecord` |

---

## Error Mapping

| `AccountError` variant | gRPC status code |
|------------------------|-----------------|
| `AccountNotFound` | `NOT_FOUND` |
| `IdentityAlreadyRegistered` | `ALREADY_EXISTS` |
| `EmailAlreadyRegistered` | `ALREADY_EXISTS` |
| `ConcurrentModification` | `ABORTED` (retryable) |
| `AccountNotActive` | `FAILED_PRECONDITION` |
| `InvalidStatusTransition` | `FAILED_PRECONDITION` |
| `InvalidKycTransition` | `FAILED_PRECONDITION` |
| `MfaAlreadyEnrolled` | `ALREADY_EXISTS` |
| `MfaNotEnrolled` | `FAILED_PRECONDITION` |
| `GdprDeletionAlreadyRequested` | `ALREADY_EXISTS` |
| `AccountAlreadyAnonymized` | `FAILED_PRECONDITION` |
| `RoleAlreadyAssigned` | `ALREADY_EXISTS` |
| `RoleNotAssigned` | `NOT_FOUND` |
| `EmailAlreadyVerified` | `ALREADY_EXISTS` |
| `InvalidAccountRole` / `InvalidKycStatus` / `InvalidAccountStatus` | `INVALID_ARGUMENT` |
| `Validation` | `INVALID_ARGUMENT` |
| `Storage` | `UNAVAILABLE` |

---

## Handler Defaults

Two proto request messages omit fields that exist in the underlying CQRS
command — the gRPC handler applies these hard-coded defaults:

| RPC | Missing proto field | Default applied |
|-----|--------------------|-----------------| 
| `RecordFailedLogin` | `max_attempts` | `5` |
| `RecordFailedLogin` | `lockout_duration_secs` | `900` (15 min) |
| `RequestGdprDeletion` | `retention_days` | `30` |
| `EnrollMfa` | `recovery_code_hashes` | `[]` (generated server-side) |

---

## Dependencies

| Crate | Role |
|-------|------|
| `cqrs` | Command/query bus and envelope types |
| `postgres-storage` | `StorageError`, `TransactionManager`, `run_on_shard` |
| `validation` | Field-level validation errors |
| `auth-context` | JWT principal propagation |
| `telemetry` | OpenTelemetry tracing integration |
| `transport` | gRPC channel configuration |
| `tonic` / `prost` | gRPC server and protobuf codegen |
| `uuid` (v7) | Time-sortable primary keys |
| `chrono` | UTC timestamps |

---

## 🚀 Deployment

Library-only: implements [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `account::service::AccountService` (`build` constructs the PostgreSQL pool via
`PgPoolBuilder` and wires the CQRS buses; `register` adds the gRPC + reflection
services; `health_probes` checks Postgres — the pool is `Arc`-backed, so the probe
shares it). The deployable binary is `crates/apps/account-server`:

```rust
use std::net::SocketAddr;
use account::service::AccountService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("ACCOUNT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50059".to_owned())
        .parse()?;
    service_runtime::serve::<AccountService>(addr).await
}
```
