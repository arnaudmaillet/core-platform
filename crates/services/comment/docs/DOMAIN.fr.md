---
i18n:
  source: ./DOMAIN.md
  source_sha256: 7bd89c4524acfa76a8d354ca4ae4a9f1ab76d2145e7a96aa746398bc106be668
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `comment` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Comments — réponses en fil sur les posts |
> | **Classe de sous-domaine** | **Core** — les commentaires sont un engagement UGC primaire |
> | **System of …** | **Record** pour les commentaires et leur cycle de vie |
> | **Racine(s) d'agrégat** | `Comment` (`domain`) |
> | **Tier** | **TIER-1** |
> | **Posture de défaillance** | **Fail-closed en écriture** (un commentaire posté doit persister) |
> | **Contextes amont** | clients utilisateur ; `post` (l'entité commentée) |
> | **Contextes aval** | `notification`, `engagement`, `counter` — via **Published Language** (`comment.created` / `comment.deleted`) |
> | **Journal de décisions** | [`ADR-0007`](../../../../docs/adr/0007-comment-flat-thread-two-table-tombstone-purge.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `comment` est l'autorité pour **les commentaires** : il répond à
**« qu'a-t-on commenté sur quel post, par qui, et est-ce encore présent ou retiré ? »**

**Le problème difficile.** Stocker un flux de commentaires à forte écriture avec un modèle de fil
plat à bas coût, et distinguer le *tombstone* (« supprimé » visible) du *purge* (retrait dur) pour
modération/RGPD — via un **sentinelle nil-UUID** pour l'arbre plat et un layout ScyllaDB à deux
tables (LCS pour les lookups + TWCS pour le flux temporel).

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Posséder le post commenté → `post`.
- ❌ Compter les commentaires pour l'affichage → c'est `counter` (magnitudes) ; comment émet les événements.
- ❌ Décider la modération → `moderation` décide ; comment applique la suppression.

---

## 2. Langage Omniprésent

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Comment | Une réponse attachée à un post | `Comment`, `CommentId` |
| Comment body | Le contenu textuel | `CommentBody` |
| GIF attachment | Une référence GIF attachée | `GifAttachment` |
| Comment status | État actif / tombstoné | `CommentStatus` |
| Deletion strategy | Tombstone vs purge | `DeletionStrategy` |
| Nil-UUID sentinel | Le marqueur de racine de l'arbre plat (sans parent) | (UUID nil) |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `Comment` | racine d'agrégat | Intégrité du commentaire + transitions d'état |
| `CommentBody` / `GifAttachment` | VO | Validité du contenu à la construction |
| `CommentStatus` | enum | Légalité actif → tombstoné |
| `DeletionStrategy` | enum | Tombstone (visible) vs purge (dur) |
| `PostId` / `ProfileId` | VO | Références entité commentée et auteur |

**Cycle de vie :**

```
created --(delete: tombstone)--> tombstoned   |   created --(delete: purge)--> purged (hard removed)
```

> **Transitions légales uniquement.** La stratégie de suppression est explicite ; un tombstone
> préserve l'emplacement, un purge retire la ligne (modération/RGPD).

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité pour :**
- Les commentaires — **ScyllaDB** deux tables (lookups LCS + flux TWCS). Aucun autre service ne les écrit.

**La liste « ne-pas-écrire » :** comment n'écrit jamais l'état du post ni les *comptes* de commentaires
(ceux-ci sont dérivés dans `counter` depuis les événements de comment).

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | Un commentaire référence un post + un auteur valides | domaine | `CMT-1xxx` |
| I2 | Tombstone vs purge est un choix explicite, irréversible | domaine | `CMT-1xxx` |
| I3 | L'arbre plat utilise le sentinelle nil-UUID pour les commentaires sans racine | domaine | `CMT-1xxx` |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**Créer.** Création autorisée → écriture dans les deux tables Scylla → publier `comment.created`
(consommé par `notification`, `engagement`/`counter` pour les comptes).

**Supprimer.** Tombstone (marqueur supprimé visible) ou purge (retrait dur pour modération/RGPD) →
publier `comment.deleted`.

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `post` | amont | Customer/Supplier | référence `PostId` | commentaires orphelins si la sémantique du post change |
| `notification` | aval | Published Language | `comment.created` | notifications de réponse |
| `engagement` / `counter` | aval | Published Language | `comment.created` / `comment.deleted` | comptes de commentaires |

---

## 8. Événements de Domaine (sémantique, pas wire)

| Événement | Signifie | Émis quand | Qui réagit |
|---|---|---|---|
| `comment.created` | un commentaire a été posté sur un post | la création commite | `notification` (notifier l'auteur), `counter`/`engagement` (compte++) |
| `comment.deleted` | un commentaire a été tombstoné ou purgé | la suppression commite | `counter`/`engagement` (compte--), fils |

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| Arbre plat sentinelle nil-UUID + layout Scylla deux tables (LCS+TWCS) | [`ADR-0007`](../../../../docs/adr/0007-comment-flat-thread-two-table-tombstone-purge.md) | Accepté |
| Sémantique de suppression tombstone vs purge | [`ADR-0007`](../../../../docs/adr/0007-comment-flat-thread-two-table-tombstone-purge.md) | Accepté |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Core — les commentaires sont du UGC primaire.
- **Volatilité :** faible-à-moyenne.
- **Dette de modélisation connue :** fil plat uniquement (pas de threading imbriqué).
- **Capacités différées :** fils imbriqués ; pièces jointes plus riches.
