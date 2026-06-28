---
i18n:
  source: ./0016-social-graph-four-table-scylla-logged-batch.md
  source_sha256: 0158a446574f81f1bb8403089f02acaa132be52c603086b758482859bef7446b
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0016-social-graph-four-table-scylla-logged-batch.md`](./0016-social-graph-four-table-scylla-logged-batch.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0016 : Social-graph utilise un schéma Scylla 4 tables avec double-écritures logged-batch atomiques

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** social-graph ; aval timeline, counter ; profile (tier)
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Une relation est intrinsèquement bidirectionnelle à requêter — « qui je suis » et « qui me suit » —
donc chaque follow/block nécessite un index avant et un index inverse. Si ces deux écritures peuvent
diverger, le graphe se corrompt (un follow visible d'un côté mais pas de l'autre). Les lectures
chaudes (fan-out de timeline) ont besoin de l'ensemble des followers rapidement, et un block doit
sectionner atomiquement les follows existants des deux côtés. Le tier d'auteur dérive du nombre de
followers mais est *présenté* sur le profil.

## Décision

`social-graph` est le **système de référence des relations** sur un **schéma ScyllaDB 4 tables** (index
avant + inverse pour follows et blocks) avec des **Redis hot Sets** pour les lectures de followers, et
écrit les lignes avant + inverse en **logged batch** pour que la double-écriture soit atomique. Un
block sectionne les follows existants (`SeveredFollows`). **Le tier d'auteur est calculé ici** depuis
le nombre de followers franchissant `TierThresholds`, mais **possédé et émis par `profile`**
(ADR-0014). `timeline`/`counter` lisent le graphe via gRPC (le producteur Kafka `social-graph.follows`
est différé).

## Conséquences

- **Positives :** les index avant/inverse ne peuvent diverger ; les lectures de followers sont
  chaudes ; les blocks sont cohérents ; le calcul du tier vit là où sont les données de followers.
- **Négatives / compromis accepté :** les logged batches coûtent plus que des écritures indépendantes ;
  la réconciliation des orphelins reste une dette suivie ; les consommateurs lisent via gRPC jusqu'à
  ce que le stream de follows arrive.
- **Clôt :** le risque de corruption par index divergents et les lectures de followers lentes.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Table unique avant-seulement | Les requêtes inverses (« qui me suit ») deviennent des full scans |
| Écritures avant/inverse non-atomiques | Les index divergent → corruption du graphe |
| Émettre le tier depuis social-graph | Le tier est *présenté* sur le profil (ADR-0014) |
