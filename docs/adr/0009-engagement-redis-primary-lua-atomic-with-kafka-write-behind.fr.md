---
i18n:
  source: ./0009-engagement-redis-primary-lua-atomic-with-kafka-write-behind.md
  source_sha256: 083a3f2195e869e8236d265b3162ee0e7001505530984f383c59114136ca3ba0
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0009-engagement-redis-primary-lua-atomic-with-kafka-write-behind.md`](./0009-engagement-redis-primary-lua-atomic-with-kafka-write-behind.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0009 : Engagement est Redis-primary avec des arêtes Lua-atomiques et durabilité Kafka write-behind

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** engagement ; counter (magnitudes) ; notification, geo-discovery
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Les réactions sont des bascules extrêmement fréquentes et idempotentes (like/unlike). Un aller-retour
base par bascule ne peut suivre, et un read-modify-write non-atomique court sous concurrence
(double-likes, unlikes perdus). Mais les réactions restent une **arête de référence** (« qui a réagi,
comment ») qui doit survivre à une perte de cache.

## Décision

Engagement est **Redis-primary** : chaque react/unreact est une pose/effacement **Lua-atomique** de
l'arête de réaction plus une mise à jour du score in-Redis (idempotent par construction), avec
**Kafka write-behind** pour l'enregistrement durable et la propagation aval (`engagement.reactions`,
`engagement.score_updated`). Engagement possède l'**arête** ; `counter` possède les **magnitudes**
dérivées (voir ADR-0008). L'arête est la vérité ; le score est dérivé des `ReactionWeight`.

## Conséquences

- **Positives :** les bascules sur le hot path sont atomiques et rapides sans aller-retour base ; la
  durabilité est préservée de façon asynchrone ; les magnitudes sont l'affaire de quelqu'un d'autre.
- **Négatives / compromis accepté :** une fenêtre de lag write-behind où l'enregistrement durable
  traîne derrière Redis ; la réconciliation/le rejeu dépend du stream Kafka.
- **Clôt :** le goulot d'aller-retour base par bascule et les conditions de course sur les réactions.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Réactions base-primary | L'aller-retour par bascule ne peut soutenir le volume de réactions |
| Read-modify-write Redis non-atomique | Court sous concurrence (réactions doubles/perdues) |
| Garder les magnitudes ici aussi | Le comptage appartient à `counter` (ADR-0008) |
