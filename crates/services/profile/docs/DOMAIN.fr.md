---
i18n:
  source: ./DOMAIN.md
  source_sha256: 5a3681e03ed848d96039cfa5ab9145886826d8de5dbfa1b525b388318b7dd660
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `profile` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Profile — le persona public au-dessus d'un compte |
> | **Classe de sous-domaine** | **Supporting** — la couche de présentation de l'identité ; visible côté produit mais dérivée de `account` |
> | **System of …** | **Record** pour le persona public (handle, display name, bio, avatar, tier, visibilité) |
> | **Racine(s) d'agrégat** | `Profile` (`domain`) |
> | **Tier** | **TIER-0** |
> | **Posture de défaillance** | **Fail-closed en écriture** — les changements de persona (surtout le handle) doivent être cohérents |
> | **Contextes amont** | `account` (cycle de vie du compte) ; `social-graph` (source du tier d'auteur) ; clients (éditions) |
> | **Contextes aval** | `post`, `search`, `geo-discovery`, `timeline` — via **Published Language** (`profile.v1.events`) |
> | **Journal de décisions** | [`ADR-0014`](../../../../docs/adr/0014-profile-public-persona-with-dual-axis-visibility.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `profile` est l'autorité pour **le persona public** : il répond à
**« quel est le handle, display name, bio, avatar, tier et visibilité de cet utilisateur ? »**

**Le problème difficile.** Posséder un **handle** globalement unique et revendicable sans conflit sous
concurrence, plus une visibilité bi-axiale (choix du propriétaire **et** masquage de modération), tout
en étant la source de dénormalisation que d'autres read-models intègrent.

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Posséder le compte/identifiants/PII → `account` est le SoR ; profile est la face publique.
- ❌ Calculer le tier d'auteur → le consomme (l'initiative tier social-graph→profile) ; profile *possède et émet* le tier.
- ❌ Construire feeds/recherche → émet `profile.v1.events` que ceux-ci consomment.

---

## 2. Langage Omniprésent

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Profile | L'enregistrement de persona public | `Profile`, `ProfileId` |
| Handle | Le @nom globalement unique et revendicable | `Handle`, `HandleChanged` |
| Display name / bio / avatar / banner | Champs de présentation | `DisplayName`, `Bio`, `AvatarUrl`, `BannerUrl` |
| Visibility | Visibilité choisie par le propriétaire | `ProfileVisibility` |
| Masking reason | Pourquoi la modération a masqué un profil | `MaskingReason`, `ProfileHidden` |
| Verification | État du badge vérifié | `VerificationKind`, `ProfileVerified` |
| Tier | Le tier d'auteur (dénormalisé + émis) | `TierChanged` |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `Profile` | racine d'agrégat | Cohérence du persona + unicité du handle |
| `Handle` | VO | Unicité globale ; revendication sous concurrence |
| `DisplayName` / `Bio` / `AvatarUrl` / `BannerUrl` / `WebsiteUrl` / `Locale` | VO | Validité des champs à la construction |
| `ProfileVisibility` / `ProfileStatus` / `ProfileKind` / `VerificationKind` | enum | Vocabulaires fermés visibilité/status/kind/vérification |

**Cycle de vie :**

```
created --(update / verify)--> active --(hide: owner OR moderation)--> hidden --(restore)--> active --> deleted
```

> **Transitions légales uniquement.** La visibilité est **bi-axiale** — un profil n'est visible que si
> le propriétaire *et* la modération l'autorisent ; un changement de handle est un événement, pas un
> renommage silencieux.

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité pour :**
- Le persona public (handle, noms, bio, URL média, tier, visibilité) — **ScyllaDB** + **Redis** (cache d'entité L1). Aucun autre service n'écrit les champs de persona.

**Ce contexte détient des copies qu'il ne possède PAS :**

| Donnée copiée | Possédée par | Maintenue fraîche via | Tolérance d'obsolescence |
|---|---|---|---|
| État du cycle de vie du compte | `account` | `account.v1.events` (DLQ `account.v1.events.dlq`) | cohérence à terme |
| Tier d'auteur (calculé) | `social-graph` (calcule) | flux de changement de tier | cohérence à terme |

**La liste « ne-pas-écrire » :** profile n'écrit jamais le compte/la PII, et ne construit jamais les read-models qui consomment ses événements.

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | Les handles sont globalement uniques ; les revendications sont sans conflit sous concurrence | domaine + store | `PRF-1xxx` (conflit de revendication) |
| I2 | Visibilité bi-axiale — propriétaire ET modération doivent tous deux autoriser | domaine | `PRF-1xxx` |
| I3 | Un changement de persona émet le `profile.v1.events` correspondant | domaine (après-save) | — |
| I4 | Les modifications concurrentes sont détectées | domaine | `PRF-1xxx` (concurrent_modification) |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**Provisionner / mettre à jour.** Consommer `account.v1.events` pour provisionner un persona ; les
éditions client mutent le `Profile` et émettent `profile.v1.events` (incl. `HandleChanged`,
`ProfileVerified`, `TierChanged`).

**Revendication de handle.** Une revendication vérifie l'unicité globale atomiquement ; les conflits
retournent une erreur de conflit de revendication (suivie par
`profile.handle.claim.conflict_total`).

**Tier.** `social-graph` calcule le tier depuis le nombre de followers → profile possède et émet
`profile.tier_changed` → `geo-discovery`/`timeline` s'allument (consommateurs prêts). *(Côté
producteur selon l'initiative author-tier.)*

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `account` | amont | ACL | `account.v1.events` | le provisionnement du persona casse |
| `social-graph` | amont | Customer/Supplier | calcul du tier | l'émission du tier casse |
| `post`/`search`/`geo-discovery`/`timeline` | aval | Published Language (OHS) | `profile.v1.events` | instantanés d'auteur / indexation cassent |

> **Anti-Corruption Layer :** le consumer d'événements `account` traduit le cycle de vie du compte en
> provisionnement de persona.

---

## 8. Événements de Domaine (sémantique, pas wire)

| Événement (`profile.v1.events`) | Signifie | Émis quand | Qui réagit |
|---|---|---|---|
| `profile_created` / `profile_updated` | persona créé/édité | la commande commite | `post`/`search` (instantanés) |
| `handle_changed` | le @handle a changé | revendication de handle | `search` (ré-indexation), intégrations |
| `profile_verified` | le badge de vérification a changé | vérification | `search`, intégrations |
| `tier_changed` | le tier d'auteur a changé | recalcul du tier | `geo-discovery`, `timeline` |
| `profile_hidden` / `profile_restored` / `profile_deleted` | visibilité/cycle de vie | action propriétaire/modération | read-models (démantèlement/restauration) |

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| Profile est le persona public au-dessus du SoR `account` ; émet `profile.v1.events` | [`ADR-0014`](../../../../docs/adr/0014-profile-public-persona-with-dual-axis-visibility.md) | Accepté |
| Visibilité bi-axiale (propriétaire ET modération) | [`ADR-0014`](../../../../docs/adr/0014-profile-public-persona-with-dual-axis-visibility.md) | Accepté |
| Tier d'auteur : social-graph calcule → profile possède + émet | _ouvert — initiative author-tier_ | Cadré |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Supporting — la présentation côté produit de l'identité, dérivée de `account`.
- **Volatilité :** moyenne — les champs de persona et la vérification évoluent.
- **Dette de modélisation connue :** le côté producteur du tier d'auteur (social-graph→profile) est cadré, pas entièrement construit.
- **Capacités différées :** flux de vérification plus riches ; déploiement d'indexation d'événements de profil vers `search`.
