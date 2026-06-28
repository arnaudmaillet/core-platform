---
i18n:
  source: ./README.md
  source_sha256: fbc0f03fb12485e283da98ceeac6b1fb7eb32bf8bcc056284974e6da6d29774c
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les noms de fichiers, identifiants, topics et noms de types
> restent en anglais.

# Architecture (C4) — régénérée

Ce répertoire contient le **modèle C4 corrigé**, exprimé comme un workspace Structurizr
([`workspace.dsl`](./workspace.dsl)).

> **Artefact dérivé.** Ce modèle est *généré depuis* la documentation domaine — ce n'est pas une
> source de vérité. La vérité est le code plus [`docs/domain/CONTEXT_MAP.md`](../domain/CONTEXT_MAP.md)
> et les Domain Cards par service (`crates/services/<svc>/docs/DOMAIN.md`). Si le diagramme
> diverge de ceux-ci, le diagramme est périmé — le régénérer, ne pas s'y fier.

Il **supersède** le modèle pré-flotte (depuis supprimé), qui décrivait une architecture jamais
livrée — services fantômes, mauvaise stack, 7 services manquants.

## Ce qui est modélisé

- **System Context** — la plateforme et ses dépendances externes (Keycloak, S3/MinIO, CloudFront, APNs/FCM, KMS/témoin).
- **Containers** — les 17 services (realtime scindé en gateway + dispatcher), les technologies de datastore partagées (ScyllaDB / PostgreSQL / Redis / OpenSearch) et le backbone d'événements Kafka.
- **Dynamic** — le flux « post publié → fan-out » comme exemple travaillé.
- La forme du service encode le rôle (service / worker / edge) ; la couleur encode la classe de sous-domaine (**Core** rouge, **Supporting** bleu), les deux tirés de `CONTEXT_MAP.md`. Les arêtes async (via Kafka) sont en pointillés ; les arêtes gRPC sync sont pleines.

## Choix de modélisation

- **Un container par service.** Les services à deux binaires (audit, counter, search, notification) sont notés dans leur description ; `realtime` est scindé en `realtime-gateway` et `realtime-dispatcher` car l'edge WebSocket public est architecturalement distinct.
- **Les datastores sont un container par technologie.** L'isolation par-service (keyspace / base / namespace) vit dans chaque Domain Card, pas ici.
- **L'async route via Kafka.** Les arêtes producteur→Kafka et Kafka→consommateur portent les noms de topics ; la *sémantique* producteur→consommateur vit dans [`EVENT_CATALOG.md`](../domain/EVENT_CATALOG.md).
- Le trafic API client synchrone (lecture/écriture) entre via l'ingress de la plateforme (voir [`docs/infrastructure`](../infrastructure/README.md)) et n'est pas énuméré, pour garder la vue container lisible.

## Rendu

Rendre avec le [Structurizr CLI](https://structurizr.com/help/cli) ou en important `workspace.dsl`
dans [Structurizr Lite](https://structurizr.com/lite) :

```bash
docker run -it --rm -p 8080:8080 -v "$PWD/docs/architecture:/usr/local/structurizr" structurizr/lite
```

## Le garder vrai

Quand `CONTEXT_MAP.md` ou une Domain Card change une relation, un store ou une classe de
sous-domaine, mettre à jour `workspace.dsl` dans le même changement. Le modèle est petit et
maintenu à la main à dessein ; un futur générateur pourrait l'émettre depuis les Domain Cards + la
garde de registre de topologie d'événements.

> 🇬🇧 Source anglaise : [`README.md`](./README.md).
