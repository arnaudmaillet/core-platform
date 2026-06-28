---
i18n:
  source: ./0008-counter-magnitudes-are-a-reconcilable-soref.md
  source_sha256: c13377d2ef4154091a23ec4e78d76167979380236efa92cdadf29b07f228d9a4
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0008-counter-magnitudes-are-a-reconcilable-soref.md`](./0008-counter-magnitudes-are-a-reconcilable-soref.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0008 : Counter est un System-of-Reference réconciliable pour les magnitudes, distinct de l'état d'arête

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** counter ; supersède les comptes bruts d'engagement ; producteur pour search, realtime
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

« Combien de vues/likes/followers ? » (une **magnitude**) et « qui a liké/suivi qui ? » (**état
d'arête**) semblent liés mais ont des formes opposées : les magnitudes sont un agrégat à l'échelle
firehose qui tolère l'approximation et la réconciliation ; l'état d'arête est une vérité
relationnelle exacte. Stocker les magnitudes dans les services d'état d'arête (engagement,
social-graph) couple une charge de comptage à forte écriture au write path relationnel et disperse le
même compte à travers les services.

## Décision

`counter` est un **System of Reference dédié aux magnitudes** — exact-mais-réconciliable, jamais
l'état d'arête. Il ingère un firehose pur-Kafka avec pré-agrégation fenêtrée N→1, stocke à travers un
layout hot/warm/cold 3-tiers (Redis / Postgres SoRef+réconciliation / Scylla TWCS), utilise HLL pour
les comptes uniques et CMS pour les tendances, et garde tous les effets de bord multi-tiers derrière
un **ledger d'idempotence clé par `WindowId`** pour que les rejeux ne puissent double-compter. Il
**supersède les comptes bruts de vues/partages d'engagement** et **réconcilie** les totaux faisant
autorité face aux SoR propriétaires (dérive → `CTR-5002`). Les lectures **échouent ouvertes** (timeout
dur → périmé/approximatif).

## Conséquences

- **Positives :** le comptage scale indépendamment des SoR d'arête ; un seul foyer par magnitude ;
  sûr au rejeu ; les lectures hot ne bloquent jamais le produit.
- **Négatives / compromis accepté :** les magnitudes sont cohérentes-à-terme et réconciliées, pas
  transactionnellement exactes à l'instant de la lecture ; les sources de réconciliation (ex. comptes
  de followers) doivent être exposées par les SoR d'arête.
- **Clôt :** les compteurs dispersés et le couplage comptage-à-forte-écriture-sur-le-write-path-relationnel.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Garder les comptes dans engagement/social-graph | Couple le comptage firehose au write path relationnel ; duplique les comptes |
| Compteurs transactionnels exacts | Ne scale pas au volume firehose ; fait fondre le hot path |
| Approximatif-seulement (sans réconciliation) | Dérive sans borne de la vérité d'arête faisant autorité |
