---
i18n:
  source: ./DOMAIN.md
  source_sha256: c3a6f05972f81abda4eec79aebd5b7bf10ef456803138a1227b5c2b6562c00c0
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `engagement` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Engagement — réactions et leur état d'arête / scoring |
> | **Classe de sous-domaine** | **Core** — interaction directe de l'utilisateur avec le contenu ; l'arête de réaction est du tissu produit |
> | **System of …** | **Record** pour l'état d'arête de réaction (« qui a réagi, comment ») ; les magnitudes sont superseded par `counter` |
> | **Racine(s) d'agrégat** | `Reaction` (`domain`) |
> | **Tier** | **TIER-1** |
> | **Posture de défaillance** | **Fail-open-ish** — Redis-primary avec atomicité Lua, Kafka write-behind |
> | **Contextes amont** | clients utilisateur ; `comment` (comptes) |
> | **Contextes aval** | `counter` (magnitudes), `notification`, `geo-discovery` (score) — via **Published Language** |
> | **Journal de décisions** | [`ADR-0009`](../../../../docs/adr/0009-engagement-redis-primary-lua-atomic-with-kafka-write-behind.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `engagement` est l'autorité pour **les réactions** : il répond à
**« qui a réagi à ce contenu, avec quelle réaction, et quel est le score d'engagement pondéré ? »**

**Le problème difficile.** Appliquer atomiquement des bascules de réaction idempotentes à haute
fréquence sur le hot path — **Redis-primary avec atomicité Lua** — tout en enregistrant durablement
l'arête via **Kafka write-behind**, sans aller-retour base par bascule.

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Servir des *comptes* d'affichage → `counter` possède les magnitudes ; engagement émet les événements d'arête.
- ❌ Posséder le contenu réagi → `post` / `comment`.
- ❌ Posséder les compteurs bruts de vues/partages → superseded par `counter`.

---

## 2. Langage Omniprésent

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Reaction | L'arête de réaction d'un utilisateur sur un item de contenu | `Reaction`, `ReactionKind` |
| Reaction weight | Le poids de score d'un type de réaction | `ReactionWeight` |
| Upsert / remove | Pose/effacement idempotent d'une réaction | `ReactionUpsertedEvent`, `ReactionRemovedEvent` |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `Reaction` | racine d'agrégat | Une arête de réaction par (utilisateur, contenu) ; bascule idempotente |
| `ReactionKind` | enum | Vocabulaire de réaction fermé |
| `ReactionWeight` | VO | Contribution au scoring par type |
| `PostId` / `ProfileId` | VO | Le contenu réagi + le réacteur |

**Cycle de vie :**

```
(none) --(react)--> upserted --(react again, same kind)--> idempotent --(unreact)--> removed
```

> **Transitions légales uniquement.** Réappliquer la même réaction est idempotent (Lua-atomique dans
> Redis) ; l'arête est la vérité, le score est dérivé.

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité pour :**
- L'état d'arête de réaction — **Redis** (primary, Lua-atomique) avec **ScyllaDB** write-behind durable.

**La liste « ne-pas-écrire » :** engagement n'écrit pas les comptes d'affichage (émet des événements ;
`counter` agrège), et ne possède pas le contenu.

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | Une arête de réaction par (utilisateur, contenu) ; les bascules sont idempotentes | domaine + Lua-atomique dans Redis | `ENG-2xxx` |
| I2 | L'arête fait autorité ; le score est dérivé des poids | domaine | `ENG-3xxx` |
| I3 | Enregistrement durable via Kafka write-behind (pas d'aller-retour base par bascule) | application | `ENG-5xxx` |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**React / unreact.** Un script Lua pose/efface atomiquement l'arête de réaction et met à jour le score
in-Redis ; un `ReactionUpsertedEvent` / `ReactionRemovedEvent` est émis (Kafka write-behind) pour
l'enregistrement durable et la consommation aval.

**Propagation du score.** `engagement.score_updated` porte le score pondéré vers les consommateurs
(`geo-discovery` viralité, `counter`).

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `comment` | amont | ACL | `comment.created` / `comment.deleted` | les comptes pilotés par commentaire cassent |
| `counter` | aval | Published Language | événements de réaction | les magnitudes like/réaction cassent |
| `notification` | aval | Published Language | `engagement.reactions` | les notifications de réaction cassent |
| `geo-discovery` | aval | Published Language | `engagement.score_updated` | le scoring de viralité casse |

---

## 8. Événements de Domaine (sémantique, pas wire)

| Événement | Signifie | Émis quand | Qui réagit |
|---|---|---|---|
| `engagement.reactions` (`ReactionUpserted`/`Removed`) | une arête de réaction a été posée/effacée | react/unreact commite | `notification`, `counter` |
| `engagement.score_updated` | le score d'engagement pondéré a changé | recalcul du score | `geo-discovery`, `counter` |
| `engagement.post_reactions` / `post_interaction_counters` | agrégats de réactions/interactions par post | agrégation | consommateurs aval |

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| Réactions Redis-primary Lua-atomiques + durabilité Kafka write-behind | [`ADR-0009`](../../../../docs/adr/0009-engagement-redis-primary-lua-atomic-with-kafka-write-behind.md) | Accepté |
| Engagement garde l'*arête* de réaction ; `counter` supersède les magnitudes brutes | _voir counter §4_ | Accepté |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Core — interaction directe avec le contenu.
- **Volatilité :** faible-à-moyenne — les nouveaux types de réaction sont additifs.
- **Dette de modélisation connue :** un RPC de compte de réactions pour la réconciliation `counter` n'est pas encore exposé.
- **Capacités différées :** analytics de réactions plus riches ; réglage du scoring par type.
