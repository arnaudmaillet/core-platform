---
i18n:
  source: ./0015-search-opensearch-single-store-external-versioning.md
  source_sha256: 3d128ced727eb36508feeeb9c13316b29b7ef9ccc413dfbfeb33831905ac081c
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0015-search-opensearch-single-store-external-versioning.md`](./0015-search-opensearch-single-store-external-versioning.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0015 : Search est un read-model OpenSearch avec versioning externe et voie de lecture fail-open

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** search ; amont post, profile, moderation, counter
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

La découverte nécessite un index inversé sur profils/posts/hashtags, alimenté par un flux d'événements
at-least-once et **désordonné**. Un événement en retard ou dupliqué ne doit pas écraser un document
plus récent, l'index doit respecter la visibilité propriétaire et modération, la ré-indexation ne doit
pas causer de downtime de lecture, et un cluster de recherche dégradé ne doit pas faire tomber les
surfaces appelantes.

## Décision

`search` est un **read-model (SoReference)** cohérent-à-terme sur **OpenSearch comme store canonique
unique**, avec un **côté commande pur-Kafka** et un **RPC de lecture stateless**. Les écritures
désordonnées sont gardées par une **`DocVersion` externe** (un script Painless à 2 versions rejette
les updates périmées). Les résultats honorent l'**autorité de visibilité duale** (propriétaire +
modération). La ré-indexation est **blue-green** (construire un nouvel index, basculer l'alias
atomiquement — pas de downtime de lecture). La voie de lecture **échoue ouverte** (dégrade vers
moins/des hits plus périmés, jamais d'erreur).

## Conséquences

- **Positives :** les événements désordonnés ne peuvent régresser un document ; pas de downtime de
  lecture à la ré-indexation ; la visibilité est imposée au moment de la requête ; un cluster dégradé
  dégrade la recherche, pas le produit.
- **Négatives / compromis accepté :** la recherche est cohérente-à-terme ; OpenSearch est un store
  unique (pas de seconde source de vérité — il est reconstructible depuis l'amont à la place).
- **Clôt :** les courses d'écrasement-périmé, le downtime de ré-indexation et le blast radius de panne
  de recherche.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Faire confiance à l'ordre des événements | At-least-once + réordonnancement corrompt l'index |
| Ré-indexation en place | Downtime de lecture pendant la reconstruction |
| Lectures fail-closed | Une panne de recherche casserait les surfaces appelantes |
