---
i18n:
  source: ./DOMAIN.md
  source_sha256: 5601ef65a66972fb9e34c5dfe6ae0d972f1ca7bd36bd933effbef7bb8136d21e
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `search` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Search & Discovery — le read-model d'index inversé |
> | **Classe de sous-domaine** | **Supporting** — un read-model de découverte dérivé ; ne possède aucun contenu source |
> | **System of …** | **Reference (SoRef)** — index cohérent-à-terme sur profils/posts/hashtags, reconstructible |
> | **Racine(s) d'agrégat** | `IndexDocument` (`PostDoc` / `ProfileDoc` / `HashtagDoc`) |
> | **Tier** | **TIER-1** |
> | **Posture de défaillance** | **Fail-open** — un index dégradé retourne moins/des hits plus périmés, jamais une erreur |
> | **Contextes amont** | `post`, `profile`, `moderation` — via **ACL** sur Kafka |
> | **Contextes aval** | clients (search/suggest) ; ne publie rien de référence |
> | **Journal de décisions** | [`ADR-0015`](../../../../docs/adr/0015-search-opensearch-single-store-external-versioning.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `search` est l'autorité pour **les requêtes de découverte** : il répond à
**« quels profils/posts/hashtags correspondent à cette requête, classés, en respectant la
visibilité ? »**

**Le problème difficile.** Garder un index inversé cohérent-à-terme correct sous des événements
désordonnés et de la ré-indexation — **OpenSearch comme store canonique unique avec versioning
externe** (un script Painless à garde 2-versions), un côté commande pur-Kafka et un RPC de lecture
stateless, plus de la ré-indexation blue-green.

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Posséder profils/posts → il indexe des références (SoReference, pas SoRecord).
- ❌ Décider la visibilité → il *honore* l'autorité de visibilité duale (propriétaire + modération).
- ❌ Servir des magnitudes → consomme `PopularityScore` depuis `counter`.

---

## 2. Langage Omniprésent

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Index document | Un doc cherchable (post/profile/hashtag) | `IndexDocument`, `PostDoc`, `ProfileDoc`, `HashtagDoc` |
| Doc version | La version externe gardant les updates désordonnés | `DocVersion` |
| Index mutation | Un upsert/delete à appliquer à l'index | `IndexMutation`, `EntityDeletion` |
| Search hit / results | Une correspondance classée et l'ensemble de résultats | `SearchHit`, `SearchResults`, `HitDisplay` |
| Suggestion | Une suggestion typeahead | `Suggestion`, `Suggestions`, `SuggestQuery` |
| Visibility authority | Qui peut supprimer un doc (propriétaire vs modération) | `VisibilityAuthority`, `VisibilityChange` |
| Popularity score | Le signal de classement depuis `counter` | `PopularityScore` |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `IndexDocument` | agrégat (par type) | La projection cherchable d'une entité source |
| `DocVersion` | VO | Version externe → les écritures désordonnées sont rejetées (garde 2-versions) |
| `IndexMutation` / `EntityDeletion` | VO | Le changement d'index appliqué |
| `VisibilityAuthority` / `VisibilityChange` | enum/VO | Visibilité duale (propriétaire + modération) honorée dans les résultats |
| `SearchQuery` / `SortStrategy` | VO/enum | Intention de requête + classement |

> **Invariant.** Une update avec une `DocVersion` inférieure à celle indexée est rejetée (versioning
> externe) ; un doc n'est retourné que si les deux autorités de visibilité l'autorisent.

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité (de *référence*) pour :**
- L'index inversé — **OpenSearch** (store canonique unique). Entièrement reconstructible depuis l'amont via ré-indexation blue-green.

**Ce contexte détient des copies dérivées qu'il ne possède PAS :**

| Donnée copiée | Possédée par | Maintenue fraîche via | Tolérance d'obsolescence |
|---|---|---|---|
| Contenu post/profile/hashtag | `post` / `profile` | `post.v1.events` / `profile.v1.events` | cohérence à terme |
| Visibilité de modération | `moderation` | `moderation.v1.events` | cohérence à terme |
| Popularité | `counter` | `counter.v1.popularity` (câblage différé) | cohérence à terme |

**La liste « ne-pas-écrire » :** search ne mute jamais les entités source ; il les projette.

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | Les updates désordonnées sont rejetées par la `DocVersion` externe (garde 2-versions) | infrastructure (Painless) | `SCH-2xxx` |
| I2 | Un hit n'est retourné que si la visibilité propriétaire ET modération l'autorise | domaine | `SCH-3xxx` |
| I3 | Les lectures échouent ouvertes (dégradent, jamais d'erreur) | application | `SCH-4xxx` |
| I4 | La ré-indexation est blue-green (pas de downtime de lecture) | application | `SCH-5xxx` |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**Indexer (côté commande, pur Kafka).** Consommer `post.v1.events` (hydraté) + `profile.v1.events` +
`moderation.v1.events` → construire `IndexMutation` avec une `DocVersion` externe → upsert/delete dans
OpenSearch sous la garde Painless 2-versions.

**Requête (côté lecture, stateless).** Un `SearchQuery` / `SuggestQuery` → requête OpenSearch classée
→ filtrer par visibilité duale → retourner `SearchResults` / `Suggestions`. Fail-open sur un cluster
dégradé.

**Ré-indexation.** Le `Reindexer` blue-green reconstruit dans un nouvel index et bascule l'alias
atomiquement.

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `post` | amont | ACL | `post.v1.events` (hydraté) | la recherche de posts devient périmée |
| `profile` | amont | ACL | `profile.v1.events` | la recherche de profils (déploiement en attente) |
| `moderation` | amont | ACL | `moderation.v1.events` | la suppression de visibilité casse |
| `counter` | amont | ACL | `counter.v1.popularity` | le classement par popularité (différé) |
| clients | aval | OHS | RPC search/suggest | la découverte casse |

> **Anti-Corruption Layer :** le décodage par-source (`PostEvent`/`ProfileEvent`/`ModerationEvent`)
> mappe les formes wire étrangères vers `IndexMutation`.

---

## 8. Événements de Domaine (sémantique, pas wire)

> Ne publie **rien de référence** — c'est un read-model. Il consomme les faits de
> `post`/`profile`/`moderation`/`counter`, dont les sens sont possédés par ces contextes.

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| OpenSearch store canonique unique avec garde externe 2-versions ; côté commande pur-Kafka + RPC de lecture stateless | [`ADR-0015`](../../../../docs/adr/0015-search-opensearch-single-store-external-versioning.md) | Accepté |
| Autorité de visibilité duale (propriétaire + modération) honorée au moment de la requête | [`ADR-0015`](../../../../docs/adr/0015-search-opensearch-single-store-external-versioning.md) | Accepté |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Supporting — un read-model de découverte dérivé.
- **Volatilité :** moyenne — le classement et le schéma de doc évoluent.
- **Dette de modélisation connue :** l'indexation de profils en attente du déploiement `profile.v1.events` ; le câblage de popularité différé.
- **Capacités différées :** classement plus riche ; recherche personnalisée ; résultats mixtes inter-entités.
