---
i18n:
  source: ./DOMAIN.md
  source_sha256: d44c9a4d39e4aa5c7d44da290db437f63c3c56892245929fa00eb0485822d84d
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `traffic-redis` — Contrat de Domaine & Fonctionnel

> Le backend distribué de `traffic` : il répond à *« comment des réplicas partagent-ils un unique budget de débit global à la flotte sans un aller-retour Redis par requête ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Un `QuotaBackend` qui fait que les profils `distributed` de `traffic` appliquent un budget global à la flotte via du leasing amorti (Step 2) |
> | **Couche** | `platform` — la moitié IO de `traffic` (le mécanisme pur est le crate foundation) |
> | **Classe de sous-domaine** | **Generic** — un backend de limiteur à budget loué ; le levier est l'amortissement, pas l'algèbre |
> | **Abstraction(s) primaire(s)** | `RedisLeaseBackend` + `LeaseBook` + `ClaimSource` (`traffic_redis`) |
> | **Empreinte** | IO/avec état — un cache de lease local + un script Lua de claim single-key contre Redis |
> | **Posture en cas d'échec** | **fail-soft via policy** — un échec de claim surface en `QuotaError` ; `transport` applique `on_backend_error` (dégrader ou rejeter) |
> | **Dépend de** | `traffic`, `redis-storage`, `fred` (`i-scripts`), `async-trait`, `dashmap` |
> | **Consommé par** | `transport` (câblé comme le `QuotaBackend` des profils `distributed`) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `traffic-redis` fait autorité dans la flotte pour le **budget de rate-limit distribué** : il
répond à **« quelle est la part de chaque réplica du budget global par fenêtre, rechargée uniquement quand son
lease local s'épuise ? »** — implémentant `traffic::QuotaBackend` pour que les profils `distributed` appliquent
une limite globale à la flotte sans un hop réseau par requête.

**Le problème difficile.** Un limiteur global à la flotte naïf fait un aller-retour Redis par requête — un
amplificateur de latence et de charge. `traffic-redis` loue plutôt un *chunk* (`burst` jetons) par clé par
réplica et le sert localement, ne croisant vers Redis que sur recharge ou pour découvrir une fenêtre épuisée. Le
chemin chaud touche rarement le réseau ; une fenêtre entièrement épuisée est cachée localement pour qu'un flot
hors-budget ne martèle pas Redis.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Posséder le mécanisme de limiteur / la config / la décision `check()` → ce sont `traffic` (foundation).
- ❌ Posséder le glue gRPC (extraction de clé, mapping de décision, policy `on_backend_error`) → c'est `transport`.
- ❌ Servir les profils `local` → seuls les profils `distributed` consultent le backend.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Lease | Un chunk du budget global qu'un réplica réclame et sert localement | `LeaseBook` |
| Claim source | Le seam atomique « louer N jetons pour une clé dans une fenêtre » | `ClaimSource`, `RedisClaimSource` |
| Window budget | Jetons disponibles dans une fenêtre de lease pour un rps donné | `window_budget(rps, lease_ms)` |
| Lease backend | L'impl `QuotaBackend` câblant le book à Redis | `RedisLeaseBackend` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `RedisLeaseBackend` | impl `QuotaBackend` | Câblé comme `Arc<dyn traffic::QuotaBackend>` dans `transport` ; `prune(lease_ms)` borne la mémoire |
| `LeaseBook` | algorithme pur | Le cache de lease local + la logique de budget fenêtré ; agnostique au transport et à Redis, testé unitairement |
| `ClaimSource` | trait (seam) | Le contrat de claim atomique ; une impl in-memory garde la suite unitaire hermétique |
| `RedisClaimSource` | adaptateur fin | Un script Lua single-key → cluster-slot-safe (pas de `CROSSSLOT`) |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Le *quota backend* distribué : l'algorithme de lease (`LeaseBook`), le seam de claim (`ClaimSource`), et
  l'adaptateur Redis (`RedisClaimSource` / `RedisLeaseBackend`).

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Le mécanisme de limiteur + `check()` + les types de config | `traffic` (foundation) | Ce crate n'est que le backend derrière le seam `QuotaBackend` |
| L'extraction de clé + le mapping de policy `on_backend_error` | `transport` | Le couplage transport reste dehors ; ce crate surface un `QuotaError` |

**La liste « do-not-depend-on » :** jamais `tonic`/`http`. Il dépend *vers le haut* de `traffic` (pour
`QuotaBackend`) et de `redis-storage`/`fred` pour le claim Lua.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | L'I/O backend s'amortit sur `burst` requêtes/clé/réplica (aucun hop par requête) | `LeaseBook` | amplification de latence/charge |
| I2 | Le claim atomique est un script Lua single-key (slot-safe) | `RedisClaimSource` | `CROSSSLOT` sur un Redis Cluster |
| I3 | `LeaseBook` est pur et testé contre un `ClaimSource` in-memory | structure du crate | suite unitaire non hermétique |
| I4 | `prune(lease_ms)` tourne sur un timer avec `lease_ms` égalant la fenêtre du profil | appelant (`transport`/boucle) | mémoire non bornée ou dérive de comptabilité |
| I5 | Un échec de claim est fail-soft (surfacé en `QuotaError`, pas un panic) | `RedisLeaseBackend` | échec du chemin chaud |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Chemin chaud — un check `distributed`.** `RedisLeaseBackend` consulte `LeaseBook` : si la clé a un lease local
restant, le servir (aucun réseau). Quand le lease local est épuisé, appeler `ClaimSource` → `RedisClaimSource`
lance le script Lua single-key pour réclamer `window_budget(rps, lease_ms)` jetons atomiquement ; une fenêtre
entièrement épuisée est cachée localement pour que les requêtes hors-budget suivantes soient rejetées sans
toucher Redis.

**Échec.** Une erreur de claim devient `traffic::QuotaError` ; `transport` la mappe à la policy
`on_backend_error` du profil (dégrader vers le governor local, ou rejeter). Les requêtes servies depuis un lease
local existant ne sont pas affectées par un blip Redis.

**Bornage mémoire.** `RedisLeaseBackend::prune(lease_ms)` évince les leases par clé inactifs sur un timer ;
`tracked_keys()` dimensionne la cadence. Le `lease_ms` doit égaler la fenêtre de lease des profils ou la
comptabilité dérive.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `traffic` | amont | Separated Interface | `impl QuotaBackend` | l'application du mode distribué |
| `redis-storage` / `fred` | amont | Conformist | script de claim Lua `eval` | le claim atomique |
| `transport` | aval | Separated Interface (injecté) | `with_traffic_backend(Arc<dyn QuotaBackend>)` | le limiting global à la flotte |

> **Seam de stabilité :** le contrat public du crate est `traffic::QuotaBackend` (implémenté, pas défini ici) —
> l'inversion est ce qui permet à `transport` de le câbler sans que `transport` connaisse Redis.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

N/A — aucun `tracing`/métrique propre ; la métrique de throttle est enregistrée par `transport`. Effets de bord :
un `eval` Lua Redis single-key par recharge (pas par requête) et un cache de lease `dashmap` local.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Louer-un-chunk (amorti) au lieu de check-par-requête | [`README §Architecture`](../README.md) | Accepted |
| `LeaseBook` pur derrière un seam `ClaimSource` (suite unitaire hermétique) | [`README §Architecture`](../README.md) | Accepted |
| Claim Lua single-key pour la cluster-slot-safety | [`README §Architecture`](../README.md) | Accepted |
| Fail-soft via la policy `on_backend_error` du profil | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — un backend de limiteur à budget loué ; le levier est l'amortissement qui garde
  le chemin chaud hors du réseau.
- **Stabilité :** en évolution — c'est `traffic` Step 2 ; l'application `distributed` dépend du câblage de ce crate.
- **Volatilité :** faible — l'algorithme de lease est stabilisé ; la croissance est opérationnelle (cadence de prune, observabilité).
- **Capacités différées :** des policies de dégradation plus riches et de la télémétrie par clé ; aujourd'hui la
  forme limite/décision est héritée de `traffic`.
