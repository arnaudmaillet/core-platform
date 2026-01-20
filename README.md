# Core Platform

Monorepo for the core backend services of our social network.

## Tech Stack
- **Build System:** Bazel
- **Language:** Rust
- **API:** gRPC / Protocol Buffers
- **Local Infrastructure:** Docker

## Repository Structure

- `backend/services/`: Contains the binary applications for our microservices (e.g., `account/outbox-processor`).
- `crates/`: Contains the shared Rust libraries (crates) that make up the business logic of our services.
    - `shared-kernel`: Common libraries for all services.
    - `account`: Crate for account management.
    - `profile`: Crate for user profiles.
    - `gamification`: Crate for gamification logic.
- `proto/`: API contract definitions (Protobuf). The single source of truth for our APIs.
- `docker-compose.yml`: Defines the local development environment (databases, etc.).

## Prerequisites
- Bazel >= 7.0.0 (see `.bazelversion`)
- Docker & Docker Compose