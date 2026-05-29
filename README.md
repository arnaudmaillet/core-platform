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
Local Infrastructure
To start the necessary dependencies (Postgres, Redis, ScyllaDB, Kafka) for local development:

Bash
docker-compose up -d
Repository Structure
backend/gateway/: GraphQL Gateway (BFF).

backend/services/: Entry points for microservices (API Command Servers and Workers).

crates/: Core business logic and infrastructure abstractions.

shared-kernel: Common DDD building blocks.

infra-*: Drivers and technical implementations.

*-test-utils: Specialized testing tools.

common/rust/shared-proto: Generated gRPC contracts.

infrastructure/: Terraform modules, EKS configurations, and ArgoCD manifests.

Roadmap
[ ] Stabilization of the Bazel build chain.

[ ] Implementation of Sharding at the SQLx layer.

[ ] Extension of the Gamification module.

[ ] Distributed monitoring with OpenTelemetry.
```
