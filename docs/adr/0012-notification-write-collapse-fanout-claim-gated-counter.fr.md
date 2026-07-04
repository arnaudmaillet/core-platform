---
i18n:
  source: ./0012-notification-write-collapse-fanout-claim-gated-counter.md
  source_sha256: 8906f0cdaaff40d67cc3ed00745e3a4ddcbc48cd9a3851812ad8b9c81d9d0e58
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0012-notification-write-collapse-fanout-claim-gated-counter.md`](./0012-notification-write-collapse-fanout-claim-gated-counter.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0012 : Notification utilise un fan-out write-collapse et un compteur de non-lus claim-gated idempotent

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** notification ; amont comment, engagement, post, social-graph
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Le fil d'activité d'un utilisateur a un fort fan-in : de nombreux événements (likes, commentaires,
follows) ciblent un seul utilisateur, et un naïf « une écriture de fil par événement » amplifie les
écritures et produit du spam bruyant par-événement (« 5 personnes ont aimé » devrait se réduire à une
entrée). La livraison Kafka at-least-once signifie aussi que le redelivery ne doit pas double-compter
le badge de non-lus.

## Décision

Notification est un **fil dérivé TIER-2** qui **write-collapse** le flux à fort fan-in vers le fil
par-utilisateur (3 couches + un plafond horaire), adossé à un fil d'activité TWCS. Il garantit
l'idempotence avec des **ids UUIDv5 déterministes**, un **`created_at` heure-d'événement** (pas
d'ingestion), un **compteur de non-lus claim-gated** (`SET NX` pour qu'un redelivery ne puisse
double-incrémenter), et une **coalescence d'expéditeur unique** (`SADD`). Le push live passe par le
stream broadcast gRPC ; la livraison offline est déléguée (APNs/FCM).

## Conséquences

- **Positives :** amplification d'écriture bornée ; entrées coalescées, non-spammeuses ; comptes de
  non-lus sûrs au redelivery ; les notifications manquées sont re-dérivables (fail-open).
- **Négatives / compromis accepté :** la logique de coalescence ajoute de la complexité côté Redis ;
  le fil est une vue dérivée, faisant autorité seulement pour lui-même.
- **Clôt :** l'amplification d'écriture et les badges double-comptés sous livraison at-least-once.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Une écriture de fil par événement source | Amplification d'écriture + spam bruyant par-événement |
| Incrémenter les non-lus sans claim | Le redelivery double-compte le badge |
| `created_at` heure-d'ingestion | Désordonne le fil sous lag/rejeu de consommateur |
