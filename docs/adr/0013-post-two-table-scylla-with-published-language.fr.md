---
i18n:
  source: ./0013-post-two-table-scylla-with-published-language.md
  source_sha256: 4cfed55e2c0bb070cf108fe47ee82ecd8fe38a4d2ea86949a9a3ebc520ba682d
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0013-post-two-table-scylla-with-published-language.md`](./0013-post-two-table-scylla-with-published-language.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0013 : Post utilise un layout ScyllaDB deux tables et est la source de fan-out en langage publié

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** post ; aval timeline, search, geo-discovery, counter, realtime
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Les posts sont du contenu à forte écriture lu de deux façons — par id de post et par auteur — et tout
le côté lecture de la plateforme (fil, recherche, découverte, comptes, live) dépend de savoir quand le
contenu change. Un store à accès unique ne peut servir les deux lectures à bas coût, et si les
consommateurs accèdent au store de post toute la flotte se couple à son schéma.

## Décision

`post` est le **système de référence du contenu** dans un **layout ScyllaDB à deux tables**
(`post.posts` par id + `post.posts_by_profile` par auteur), et émet **`post.v1.events`**
(`published`/`updated`/`deleted`) comme son **Open-Host Service / Published Language** — la source de
fan-out unique que consomme le côté lecture. Les enums proto mappent le tinyint domaine **+1 sans
sentinelle UNSPECIFIED**. Post dénormalise l'état auteur et modération en consommant
`profile.v1.events` / `moderation.v1.events`.

## Conséquences

- **Positives :** les deux motifs de lecture sont bon marché ; le côté lecture est découplé
  (événements, pas accès direct au store) ; un cycle de vie de contenu faisant autorité pilote
  fil/recherche/découverte/comptes/live.
- **Négatives / compromis accepté :** une cohérence d'écriture à deux tables à maintenir ;
  `post.v1.events` est un contrat d'API dur — un changement de schéma casse chaque consommateur.
- **Clôt :** le mismatch de motifs de lecture et le couplage du côté lecture au store de post.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Table posts unique | Ne peut servir efficacement les lectures par-id et par-auteur |
| Laisser les consommateurs lire le store de post | Couple tout le côté lecture au schéma de post |
| Fan-out synchrone vers les consommateurs | Met le côté lecture sur le write path |
