---
i18n:
  source: ./0010-geo-discovery-h3-grid-dual-layer-redis-topk.md
  source_sha256: aba36c04a6077f5b822a9b6fe167599d0738857c15d81b36a9e4e89e7b4c4843
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0010-geo-discovery-h3-grid-dual-layer-redis-topk.md`](./0010-geo-discovery-h3-grid-dual-layer-redis-topk.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0010 : Geo-discovery est un read-model spatial H3 grid_disk + Redis Top-K double-couche

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** geo-discovery ; amont post, engagement, profile
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Les requêtes de viewport carte doivent retourner les posts les plus pertinents dans un rectangle
mouvant à latence interactive, sur une population de posts en changement constant et qui doit
s'auto-élaguer (les posts périmés ne doivent pas s'attarder). Les requêtes lat/lng arbitraires
s'indexent mal, et une liste par-cellule sans borne coûte de la mémoire et retourne du bruit.

## Décision

Geo-discovery est un **read-model spatial fail-open (SoReference)** construit sur **H3** : un viewport
se mappe à un `H3 grid_disk` de cellules couvrantes ; chaque cellule est une **structure Redis
double-couche (ZSET + cardinalité)** maintenue par des scripts Lua **Top-K / XX / prune** avec
**rétention TTL** pour que l'index s'auto-élague. Les cartes sont projetées depuis les événements
amont (`post.published`/`post.deleted`, `engagement.score_updated`, `profile.tier_changed`) ; geo ne
possède aucune vérité source et un index dégradé retourne moins/des cartes plus périmées plutôt
qu'une erreur.

## Conséquences

- **Positives :** les requêtes de viewport deviennent un ensemble borné de lookups de cellules ; les
  résultats par-cellule sont classés et plafonnés ; la rétention est automatique via TTL ; l'index est
  entièrement reconstructible depuis l'amont.
- **Négatives / compromis accepté :** les résultats sont cohérents-à-terme et approximatifs (Top-K) ;
  dépend de l'amont émettant le payload nécessaire (voir la lacune ouverte d'enrichissement post→geo).
- **Clôt :** les problèmes d'indexation spatiale et de liste par-cellule sans borne.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Requêtes lat/lng sur un store générique | Mauvaise localité spatiale ; lent à l'échelle viewport |
| Listes par-cellule sans borne | Explosion mémoire ; retourne du bruit peu pertinent |
| Faire de geo un SoR | C'est une projection ; la durabilité vit dans `post` |
