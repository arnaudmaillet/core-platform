---
i18n:
  source: ./0017-timeline-hybrid-push-pull-fanout.md
  source_sha256: 27300f904aee5b26886c046da15f1081c7fda93ec66d4ee6d26ca70788e26a16
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0017-timeline-hybrid-push-pull-fanout.md`](./0017-timeline-hybrid-push-pull-fanout.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0017 : Timeline utilise un fan-out hybride push/pull

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** timeline ; amont post, social-graph, profile
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

La génération de fil a deux modes d'échec classiques. **Fan-out à l'écriture** (matérialiser chaque
post dans le fil de chaque follower) explose pour les auteurs à fort nombre de followers — un post de
célébrité écrit des millions de lignes. **Fan-out à la lecture** (tirer les posts de chaque followee
au moment de la lecture) explose pour les utilisateurs qui suivent beaucoup de comptes. Aucun extrême
ne scale à la fois pour les distributions d'auteurs et de lecteurs.

## Décision

`timeline` est un **read-model de fil SoReference** utilisant un **fan-out hybride push/pull** : les
auteurs tier-normal sont **poussés** (matérialisés dans les ZSETs de fil des followers à
`post.published`) ; les auteurs haut-tier sont **tirés** au moment de la lecture, et les deux sont
**fusionnés via un Lua `ZREVRANGEBYSCORE`**, paginés par `FeedCursor`. La frontière push/pull est
pilotée par `AuthorTier` (consommé depuis `profile`). L'ensemble des followers est lu depuis
`social-graph` via gRPC ; le fil est fail-open et reconstructible.

## Conséquences

- **Positives :** borne à la fois l'amplification d'écriture (pas de fan-out célébrité à des millions
  de lignes) et l'amplification de lecture (pas de pull de milliers de followees) ; l'ordonnancement
  est une seule fusion Lua.
- **Négatives / compromis accepté :** complexité de la fusion au moment de la lecture ; la justesse
  dépend du signal de tier d'auteur ; la performance du fan-out est une dette de réglage suivie.
- **Clôt :** les problèmes d'amplification d'écriture célébrité et d'amplification de lecture
  follower-lourd.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Pur fan-out à l'écriture | Les posts de célébrité écrivent des millions de lignes de fil |
| Pur fan-out à la lecture | Les utilisateurs suivant beaucoup de comptes paient un coût de lecture énorme |
| Seuil statique sans signal de tier | Mal-classe les auteurs ; nécessite l'entrée de tier `profile` |
