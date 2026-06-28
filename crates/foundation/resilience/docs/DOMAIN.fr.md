---
i18n:
  source: ./DOMAIN.md
  source_sha256: 296c08fb171e964208dde7fb81814d14592b18c404fc7ae3e08d22a37ba75c8e
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `resilience` — Contrat de Domaine & Fonctionnel

> Tolérance aux pannes en sortie : le middleware Tower pur qui répond à *« cet appel sortant doit-il être tenté, retenté, ou échoué rapidement ? »* — le miroir côté client de `traffic`.

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Protection contre les pannes en cascade à la frontière de transport sortante (circuit breaker · retry · timeout) |
> | **Couche** | `foundation` — un mécanisme de middleware Tower pur |
> | **Classe de sous-domaine** | **Generic** — patterns de résilience standard ; le levier est la cohérence à l'échelle de la flotte + le hot-reload |
> | **Abstraction(s) primaire(s)** | `ResilienceProfile` + les trois types `*Layer` (`resilience::profile`, `::{circuit_breaker, retry, timeout}`) |
> | **Empreinte** | pure (aucune IO, aucun `notify`, aucune tâche spawnée) ; feature `serde` désactivée par défaut |
> | **Posture en cas d'échec** | **fail-fast** — un downstream malsain fait sauter le circuit et les requêtes ne sont *pas* transmises (`CircuitOpen`) |
> | **Dépend de** | `tower`, `arc-swap`, `tokio`, `thiserror`, `rand`, `error`, `serde` (optionnel) |
> | **Consommé par** | `transport` (clients gRPC/Kafka), le bus `cqrs` ; configuré via `infra-config` |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `resilience` fait autorité dans la flotte pour la **tolérance aux pannes en sortie** : il répond
à **« ce downstream est-il assez sain pour être appelé, et si un appel échoue de façon transitoire, comment
retenter sans amplifier la panne ? »** — à l'échelle de la flotte, sans toucher la logique métier.

**Le problème difficile.** Trois patterns (timeout, circuit breaker, retry) doivent se composer dans un ordre
*porteur* et se reconfigurer à chaud pendant un incident, tout en restant une bibliothèque pure et testable
unitairement. `resilience` garde le mécanisme pur (profils nommés derrière des handles `ArcSwap`) et pousse le
parsing/validation/bindings dans `infra-config`, pour qu'un push de config re-règle un channel vivant sans rebuild.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Protéger un *serveur* de la charge entrante (entrée) → c'est `traffic`, le crate miroir.
- ❌ Parser ou valider sa config → relève de `infra-config`.
- ❌ Retenter au niveau du channel gRPC → les bodies HTTP/2 sont des streams ; le retry relève de la couche
  application (voir `transport`), les couches circuit/timeout enveloppent le channel.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Profile | Une classe-de-service nommée regroupant un timeout + CB + retry | `ResilienceProfile`, `ResilienceProfileSpec` |
| Circuit breaker | La machine à états fail-fast sur la santé d'un downstream | `CircuitBreakerLayer`, `CircuitBreakerConfig` |
| Retry policy | Si une erreur à la tentative N est retentable | `RetryPolicy`, `DefaultRetryPolicy` |
| Backoff | L'échéancier de délai entre tentatives | `BackoffStrategy`, `ExponentialBackoff`, `JitterKind` |
| Resilience error | L'enveloppe d'issue émise par le middleware | `ResilienceError::{CircuitOpen, Timeout, MaxRetriesExhausted, Inner}` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `ResilienceProfile` | handle runtime | Regroupe les trois couches ; timeout/CB derrière `ArcSwap` partagé pour le hot-reload |
| `ResilienceError<E>` | enveloppe d'erreur | `Inner(E)` est la *seule* variante portant l'état downstream ; le reste est émis par le middleware |
| `CircuitBreakerLayer` / `…Service` | couche Tower | Possède le `Arc<StateMachine>` ; les clones partagent l'état (tonic clone par RPC) |
| `RetryLayer` | couche Tower | Exige `S: Clone` **et** `Req: Clone` (réémet la requête par tentative) |
| `BackoffStrategy` | trait (seam) | `next_delay(attempt)` ; `ExponentialBackoff` défaut au jitter `Full` |

**Machine à états du circuit breaker.**

```
Closed --(failure_threshold échecs consécutifs)--> Open
Open   --(open_duration écoulé)--> HalfOpen
HalfOpen --(success_threshold succès)--> Closed
HalfOpen --(tout échec de probe)--> Open          (half_open_max_calls plafonne les probes concurrentes)
```

> Une requête atteignant un circuit `Open` n'est **pas transmise** — elle retourne `CircuitOpen` immédiatement.

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Les trois couches de middleware, leurs machines à états/types de config, et l'ordre de composition. L'état
  de circuit, la comptabilité de retry, et l'application du timeout vivent ici et nulle part ailleurs.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Parsing TOML / validation / bindings / surveillance fichier | `infra-config` | Frontière de pureté — le mécanisme ne lie ni `notify`/`toml` |
| Mapper `ResilienceError` → `TransportError`/`Status` | `transport` | Le couplage transport reste hors du crate pur |
| Le *sens* de retentabilité d'une erreur | `error` (`AppError::is_retryable`) | `DefaultRetryPolicy` y délègue |

**La liste « do-not-depend-on » :** jamais `notify`, `toml`, `tonic`, ou `http`. La feature `serde` (désactivée
par défaut) est la seule surface optionnelle — désactivée, le crate ne lie aucun code serde/derive.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | L'ordre des couches est Timeout ⊃ CircuitBreaker ⊃ Retry | composition (appelant) | le circuit mal-compte les retries / saute trop tard |
| I2 | La config est échantillonnée une fois par `call` (un `ArcSwap::load`) | le `call` de chaque couche | valeurs incohérentes en cours de décision |
| I3 | Un swap de config ne réinitialise jamais l'état vivant (compteurs, timers, circuit) | store `ArcSwap` | — |
| I4 | `Inner(E)` est la seule variante portant l'état d'erreur downstream | système de types | — |
| I5 | L'exposant est borné (≤30) pour éviter l'overflow `u64` du backoff | `ExponentialBackoff` | — |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Chemin chaud — un `call`.** Le `TimeoutLayer` le plus externe arme une échéance ; `CircuitBreakerLayer`
vérifie l'état (`Open` → retourne `CircuitOpen` sans transmettre) et, si `Closed`/`HalfOpen`, transmet ;
`RetryLayer` réémet sur erreurs retentables avec `ExponentialBackoff` + jitter `Full` jusqu'à `max_attempts`.
Chaque couche fait `ArcSwap::load` de sa config une fois pour que la décision raisonne sur des valeurs cohérentes.

**Pourquoi l'ordre est porteur.** Le circuit doit se situer *à l'extérieur* du retry pour compter chaque
tentative (y compris les retries) et sauter à temps ; inversez-les et le circuit ne voit que le premier appel.
Timeout le plus externe borne le budget *total* de la requête, retries inclus.

**Reconfiguration.** `infra-config` appelle `ResilienceProfile::apply(spec)` ; le store `ArcSwap` est sans
verrou et laisse l'état de circuit, les compteurs de retry, et les timers intacts — un channel vivant se re-règle
sans rebuild.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `error` | amont | Conformist | `AppError::is_retryable` (`DefaultRetryPolicy`) | la classification de retry |
| `transport` | aval | Published Contract | les trois `*Layer` + `ResilienceProfile` | chaque client gRPC/Kafka résilient |
| bus `cqrs` | aval | Published Contract | enveloppe le dispatch sortant | la résilience couche application |
| `infra-config` | aval | Conformist (`serde`) | `ResilienceProfileSpec` | le parsing/hot-reload de `[resilience]` |
| `traffic` | frère (miroir) | — | partage la forme catalog+bindings, direction opposée | symétrie |

> **Seam de stabilité :** `ResilienceError`, les types `*Layer`, et `ResilienceProfile` sont une API publique
> consommée par `transport`.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| transition de circuit | `tracing` INFO (`prev`/`next`) | la machine à états bouge | dashboards de résilience |
| circuit déclenché / probe échouée | `tracing` WARN | seuil franchi / probe half-open échoue | paging sur `CircuitOpen` |
| retry planifié / timeout de requête | `tracing` WARN (`attempt`, `delay_ms`, `timeout_ms`) | un retry est mis en file / échéance atteinte | alertes niveau warn |

Pas encore d'export de métriques OTel (un TODO noté) ; aucune mutation d'état externe.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Middleware pur ; profils derrière `ArcSwap`, parsing dans `infra-config` | [`README §Architecture`](../README.md) | Accepted |
| Ordre des couches Timeout ⊃ CircuitBreaker ⊃ Retry (le circuit compte les retries, saute à temps) | [`README §Architecture`](../README.md) | Accepted |
| Défaut `JitterKind::Full` pour vaincre les retries en lockstep à l'échelle de la flotte | [`README §Architecture`](../README.md) | Accepted |
| Aucun `RetryLayer` au niveau du channel (buffering de body HTTP/2) | [`transport README`](../../../platform/transport/README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — patterns de résilience classiques ; le levier est l'uniformité à l'échelle de
  la flotte + le re-réglage live, pas la nouveauté.
- **Stabilité :** contrat stable — prêt pour la production, aucun stub.
- **Volatilité :** faible — les trois patterns sont stabilisés ; la croissance est dans les stratégies de backoff / policies.
- **Capacités différées :** des instruments de métriques OTel pour les couches (aujourd'hui seulement des événements `tracing`).
