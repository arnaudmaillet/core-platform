---
i18n:
  source: ./DOMAIN.md
  source_sha256: f7a102b1814481eae2e1a9da33bdf387c5c891c1b204d62e904e5126a0e236a0
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `auth-context` — Contrat de Domaine & Fonctionnel

> Le traducteur de sécurité en entrée : il répond à *« qui est ce caller ? »* — convertissant un Bearer token opaque en principal typé et le propageant à travers la call-stack async.

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Authentification en entrée : vérification JWT (token → `CurrentPrincipal` typé) + propagation d'identité task-local |
> | **Couche** | `platform` — le traducteur de sécurité à la frontière entrante d'un service |
> | **Classe de sous-domaine** | **Generic** — vérification OIDC/JWT standard ; le levier est l'agnosticisme de provider + la propagation sans thread |
> | **Abstraction(s) primaire(s)** | `JwtDecoder` + `CurrentPrincipal<C>` + `ClaimsExtractor<C>` (`auth_context`) |
> | **Empreinte** | IO/avec état — une tâche de fond de refresh JWKS + un cache ; la vérification est du CPU pur |
> | **Posture en cas d'échec** | **fail-closed sur un mauvais token** (rejet) mais **fail-soft sur un IdP instable** (les clés périmées continuent de fonctionner) |
> | **Dépend de** | `jsonwebtoken`, `tokio`, `reqwest` (JWKS), `tracing`, `uuid`, `cqrs` (optionnel) |
> | **Consommé par** | les frontières entrantes des services / la gateway (tout ce qui authentifie une requête) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) ; lié : [`ADR-0005 (auth)`](../../../../docs/adr/0005-auth-federated-idp-with-platform-edge-tokens.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `auth-context` fait autorité dans la flotte pour l'**authentification de requête** : il répond à
**« qui est ce caller, et comment rendre cette identité disponible partout en aval sans la faufiler à travers
chaque signature de fonction ? »** Il fait de l'authentification (vérifier + extraire + propager), pas de la
policy d'autorisation et pas du transport.

**Le problème difficile.** La vérification de token doit être une opération de chemin chaud *CPU pur* (aucun
réseau par requête), or les clés publiques tournent et vivent à un endpoint JWKS OIDC, et l'identité résultante
doit atteindre la logique métier profonde sans polluer les signatures. `auth-context` récupère+cache les clés
dans une tâche de fond (les clés périmées continuent de marcher pendant un blip de l'IdP), garde l'extraction de
claims pluggable par IdP, et lie le principal à un `tokio::task_local!`.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Autoriser (décider ce qu'un principal peut faire) → c'est la policy du service.
- ❌ Émettre ou rafraîchir des tokens / courtier l'IdP → c'est le service `auth` (`ADR-0005`).
- ❌ Posséder le transport / la couche HTTP entrante → il convertit un token qu'un caller a déjà extrait.
- ❌ Propager l'identité à travers `tokio::spawn` automatiquement → re-binder avec `with_principal` dans la tâche.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Principal | L'identité authentifiée typée (id, tenant, permissions, claims bruts) | `CurrentPrincipal<C>`, `PrincipalId`, `Permission` |
| Claims extractor | La stratégie par IdP mappant claims bruts → un principal | `ClaimsExtractor<C>`, `OidcClaimsExtractor` |
| JWKS cache / refresher | Les clés publiques cachées + la boucle de refresh de fond | `JwksCache`, `JwksRefresher`, `JwksClient` |
| Decoder | Le point d'entrée vérifier-puis-extraire | `JwtDecoder` |
| Task-local principal | Identité liée à la call-stack async, lue sans faufilage | `with_principal`, `current_principal` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `JwtDecoder<C, E>` | vérificateur | `decode(token)` = lookup `kid` du header → `jsonwebtoken::decode` (sig + `exp`/`nbf`/`iss`/`aud`) → extract |
| `CurrentPrincipal<C>` | type valeur | Les claims bruts voyagent comme le générique `C` (identité type-safe) |
| `ClaimsExtractor<C>` | trait (seam) | Le point de stratégie unique — un nouvel IdP/flow est un nouvel extractor, pas un fork du decoder |
| `JwksCache` | cache | Lecture `RwLock` O(1) sur le chemin chaud ; `replace` swap l'ensemble de clés atomiquement |
| `with_principal` / `current_principal` | propagation | Liaison `task_local!` ; **non** héritée à travers `spawn` |
| `AuthError` | enum | La raison de rejet précise (`InvalidSignature`, `TokenExpired`, `UnknownKid`, …) |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- La vérification (signature + claims standard), la stratégie d'extraction de claims, le cache JWKS + la tâche
  de refresh de fond, et la propagation task-local.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| La policy d'autorisation (ce qu'un principal peut faire) | chaque service | Ce crate s'arrête à l'identité, pas aux décisions de permission |
| L'émission de token / le courtage IdP | service `auth` | L'émission est une préoccupation System-of-Record (`ADR-0005`) |
| L'injection d'enveloppe dans `cqrs` | gaté derrière `cqrs-integration` | Garde l'arête `cqrs` optionnelle |

**La liste « do-not-depend-on » :** jamais un crate de service ; la dépendance `cqrs` est gatée par feature
(`cqrs-integration`) pour que les appelants non-CQRS ne lient aucun `cqrs`.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | La vérification ne fait aucun appel réseau (les clés viennent du cache) | `JwtDecoder::decode` | latence du chemin chaud / couplage IdP |
| I2 | Un IdP instable ne fait pas échouer le chemin chaud (les clés périmées marchent) | boucle de backoff `JwksRefresher` | (dégradé seulement si aucune clé ne correspond) |
| I3 | Le principal task-local n'est **pas** propagé à travers `tokio::spawn` | sémantique `task_local!` | identité manquante dans la sous-tâche — re-binder |
| I4 | Un nouvel IdP/flow est un nouveau `ClaimsExtractor`, pas un changement du decoder | seam de stratégie | un fork du decoder |
| I5 | La validation `iss`/`aud` est activée par défaut en prod | `AuthContextConfig` | risque de confusion de token si désactivée |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Démarrage.** `JwksRefresher::spawn` réchauffe le cache immédiatement, puis boucle : fetch → `replace(keys)`,
délai = `refresh_interval` ; en erreur → WARN, délai = backoff (1s → `max_backoff`, ×2). Le guard du refresher
doit survivre au processus.

**Chemin chaud — par requête.** `decode_header()` lit le `kid` (sans crypto) → `JwksCache::get(kid)` (lecture
`RwLock` O(1)) → `jsonwebtoken::decode` (RS256/ES256 + `exp`/`nbf`/`iss`/`aud`, avec marge `AUTH_CLOCK_SKEW_SECS`)
→ `ClaimsExtractor::extract` → `CurrentPrincipal<C>`. Puis `with_principal(p, fut)` lie le task-local pour la
durée de la requête ; `inject_into_span()` (et optionnellement `inject_into_envelope`) surfacent l'identité vers
l'observabilité/CQRS.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| frontières entrantes / gateway | aval | Published Contract | `decode` + `with_principal` | l'authentification de requête |
| `cqrs` | amont (optionnel) | Open-Host (étend) | `inject_into_envelope` (`cqrs-integration`) | l'identité dans les métadonnées de commande |
| service `auth` | pair (runtime) | Customer/Supplier | consomme le JWKS que l'IdP publie | la rotation de clés / la validité de token |

> **Seam de stabilité :** `CurrentPrincipal<C>`, `ClaimsExtractor<C>`, `JwtDecoder`, et `AuthError` sont une API
> publique ; la règle `task_local!`-pas-à-travers-spawn est un contrat que les appelants doivent respecter.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| JWKS cache refreshed | `tracing` INFO (`key_count`, `next_refresh_secs`) | un refresh de fond réussi | dashboards d'auth |
| refresh failed | `tracing` WARN (`error`, `retry_after_secs`) | une erreur de fetch JWKS | alerte P2 si > 3/min |
| key loaded / undecodable key skipped | `tracing` DEBUG/WARN | parsing de l'ensemble JWKS | monitoring de rotation de clés |

Effets de bord : un fetch HTTP sortant par intervalle de refresh (pas par requête) ; une liaison `task_local!`
par requête. `JwksCache::is_empty()` pendant > 30s est un P1 (ne peut rien authentifier).

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| JWKS caché + refresh de fond ; la vérification est du CPU pur sur le chemin chaud | [`README §Architecture`](../README.md) | Accepted |
| `ClaimsExtractor<C>` pluggable pour l'agnosticisme de provider | [`README §Architecture`](../README.md) | Accepted |
| Propagation task-local au lieu du faufilage par signature | [`README §Architecture`](../README.md) | Accepted |
| Authentification-seulement ; l'émission/courtage est le service `auth` | [`ADR-0005`](../../../../docs/adr/0005-auth-federated-idp-with-platform-edge-tokens.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — vérification OIDC/JWT standard ; le levier est l'agnosticisme de provider et la
  propagation sans thread.
- **Stabilité :** contrat stable.
- **Volatilité :** faible — les nouveaux IdP/flows arrivent comme de nouveaux `ClaimsExtractor`, pas des changements de surface.
- **Capacités différées :** le mapping de claims service-account / machine-to-machine (`azp`/`client_id` →
  user_id) est un extractor custom, pas encore intégré.
