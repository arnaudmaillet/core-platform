# Core Platform

Monorepo centralisant les services et applications de notre réseau social.

## Stack Technique
- **Build System:** Bazel
- **Mobile:** iOS (Swift/UIKit), Android (Kotlin/Compose)
- **Backend:** Rust (Core), Go (Services), Python (ML)
- **Communication:** gRPC / Protocol Buffers

## Structure du Repo
- `/apps`: Clients mobiles et web.
- `/backend`: Microservices.
- `/proto`: Définitions des contrats d'API (Source of Truth).
- `/infra`: Déploiement et CI/CD.

## Pré-requis
- Bazel >= 7.0.0
- Docker (pour l'infra locale)