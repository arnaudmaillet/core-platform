# Core Platform

## Introduction

**Core Platform** is a high-performance backend architecture designed for massive horizontal scalability. This project implements the technical foundation of a modern social network based on a distributed microservices architecture, leveraging data **sharding** and an **Event-Driven** approach.

The architecture is strictly structured according to **Domain-Driven Design (DDD)** and **Clean Architecture** (Ports & Adapters) principles to ensure optimal maintainability and testability.

## Technical Architecture

The system is built on four fundamental pillars:

- **API Gateway (BFF):** A unified GraphQL layer (`graphql-bff`) that orchestrates calls to underlying microservices.
- **Microservices:** Autonomous services (Account, Profile, Social, Post) communicating via gRPC (Tonic).
- **Data Consistency:** Implementation of the **Transactional Outbox** pattern (via `outbox-producer` workers) to guarantee reliable inter-service events via Kafka.
- **Polyglot Persistence:**
  - **PostgreSQL (`infra-sqlx`):** Transactional data.
  - **ScyllaDB (`infra-scylla`):** High-volume data (posts, social graph).
  - **Redis (`infra-fred`):** Distributed caching and fast state management.

## Tech Stack

- **Languages:** Rust (Edition 2024, Tokio stack).
- **Communication:** gRPC (Tonic), Protocol Buffers, GraphQL (Async-graphql).
- **Infrastructure:**
  - **Cloud:** AWS (EKS).
  - **IaC:** Terraform, Terragrunt.
  - **CD/GitOps:** ArgoCD.
- **Messaging:** Apache Kafka (rdkafka).

## System Architecture (C4 Model)

The system architecture is documented using the **C4 Model** and maintained via **Structurizr**. These diagrams provide a comprehensive view of our services, boundaries, and infrastructure:

- **System Context Diagram:** High-level interactions between the platform and external systems.
- **Container Diagram:** Relationships between microservices (Gateway, Account, Profile, Social, Post) and persistent stores.
- **Cloud Infrastructure Diagram:** Detailed view of our AWS EKS deployment, VPC structure, and managed services.

> [!NOTE]
> All source files for these diagrams are located in the `/docs` directory. You can visualize them using the [Structurizr CLI](https://structurizr.com/help/cli) or by importing the `workspace.dsl` files into the [Structurizr Lite](https://structurizr.com/lite) container.

## Development & Build

### Prerequisites

- Rust (Edition 2024)
- Docker & Docker Compose
- Protoc (Protobuf Compiler)

### Build

> [!IMPORTANT]
> **Note on Bazel:** While Bazel configuration files are present in the repository, **Bazel is not yet functional** and should not be used at this stage.

**Please use Cargo exclusively for current development:**

```bash
# Build the entire workspace
cargo build

# Run tests (unit and integration via testcontainers)
cargo test
```

### Local Infrastructure

To start the necessary dependencies (Postgres, Redis, ScyllaDB, Kafka) for local development:

```bash
docker-compose up -d
```

## Repository Structure

The workspace lives under `crates/`, organised by role:

- **`crates/services/<svc>/`** — domain microservices (DDD + CQRS), **library-only**. Each exposes a composition root (`App::build`) and implements `service_runtime::Service` (`crate::service::<Svc>Service`).
- **`crates/apps/<svc>-server/`** — thin deployable binaries (one per service). Each `main` is a `service_runtime::serve::<…>(addr)` one-liner.
- **`crates/platform/service-runtime/`** — the unified fleet bootstrap: telemetry, `infrastructure.toml` load + hot-reload, ingress trace + rate-limit layers, dynamic gRPC health, and graceful shutdown. See its [README](crates/platform/service-runtime/README.md).
- **`crates/shared/`** — reusable building blocks: `error`, `cqrs`, `auth-context`, `validation`/`validate-core`, the externalized-config stack (`infra-config`, `resilience`, `traffic`, `telemetry`), `transport` (gRPC + Kafka), and storage clients under `crates/shared/storage/{postgres,scylla,redis}`.
- **`infrastructure/`** — Terraform/Terragrunt modules, EKS configuration, and ArgoCD manifests.

## Roadmap

- [ ] Stabilization of the Bazel build chain (role-tiered crate layout + a shared `contracts/` tier).
- [ ] Schema-migration runner (`apps/migrator`).
- [ ] Implementation of Sharding at the SQLx layer.
- [x] Distributed monitoring with OpenTelemetry — live log-filter and trace-sampling dials via the `[telemetry]` config section.
