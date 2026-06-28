---
i18n:
  source: ./DOMAIN.md
  source_sha256: d754558442162e438424cb2ad2ca4d0d3ca2003f33dfb6397ea13605648b2beb
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `health` — Contrat de Domaine & Fonctionnel

> Un contrat de probe en feuille du graphe : l'abstraction qui répond à *« ce backend est-il joignable maintenant ? »* — pour que les crates de stockage publient des probes que le runtime sonde, sans arête entre eux.

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Le contrat de probe liveness/readiness qui pilote le statut de santé gRPC d'un service |
> | **Couche** | `foundation` — une feuille délibérée du graphe (minuscule, ne dépend que de `async-trait` + `anyhow`) |
> | **Classe de sous-domaine** | **Generic** — une abstraction à un trait ; sa valeur est le découplage, pas le code |
> | **Abstraction(s) primaire(s)** | `HealthProbe` + `FnProbe` (`health`) |
> | **Empreinte** | pure (aucune IO, aucun spawn) — il définit le contrat, n'ouvre jamais une connexion |
> | **Posture en cas d'échec** | **fail-to-`NOT_SERVING`, auto-effaçant** — tout `Err` rétrograde le service jusqu'à ce qu'un tick ultérieur l'efface |
> | **Dépend de** | `async-trait`, `anyhow` |
> | **Consommé par** | `service-runtime` (sonde les probes → santé gRPC), crates de stockage (`scylla`/`redis`/`postgres` exposent des probes) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `health` définit le **contrat de probe** que la flotte utilise pour gater la readiness : il
répond à **« le runtime peut-il demander à n'importe quel backend "es-tu joignable ?" sans dépendre du crate
de ce backend ? »**

**Le problème difficile.** Un crate de stockage sait *comment* vérifier son backend ; le runtime sait *quand*
vérifier et *quoi* faire du résultat. Mettre le trait dans l'un ou l'autre couplerait le stockage au runtime
(ou l'inverse). Un crate feuille minuscule permet aux deux côtés de ne dépendre que de lui, pour qu'un crate de
stockage publie une probe et que le runtime la consomme avec **aucune arête entre eux** — la raison d'être même
de ce crate.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Ouvrir des connexions ou lancer de vraies requêtes → le `health::probe` du crate de stockage le fait.
- ❌ Planifier le sondage ou connaître gRPC → relève de la boucle de readiness de `service-runtime`.
- ❌ Tenir un état d'échec collant → il n'y en a pas ; la readiness se redérive à chaque tick.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Probe | Une vérification de joignabilité bon marché sondée à chaque tick | `HealthProbe` |
| Name | Identifiant court pour les logs (`"scylla"`, `"redis"`) | `HealthProbe::name` |
| Check | Le ping de joignabilité async ; `Ok` = joignable | `HealthProbe::check` |
| Fn-probe | Une probe backée par closure pour une dépendance sur mesure | `FnProbe` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `HealthProbe` | trait (seam) | `check()` doit être bon marché + idempotent (tourne à chaque tick) ; `Ok(())` = joignable, tout `Err` rétrograde |
| `FnProbe<F>` | adaptateur | Enveloppe un `Fn() -> Future` pour qu'une probe n'ait pas besoin de type dédié ; la closure est ré-invoquée à chaque tick |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Le *contrat* de probe seulement — la forme du trait et l'adaptateur closure. Rien d'autre.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Clients backend live + la vérification réelle | crates de stockage (`scylla`/`redis`/`postgres`) | Ils possèdent leur client ; ils ne dépendent que de cette feuille |
| Cadence de sondage + câblage de santé gRPC | `service-runtime` | Le runtime planifie et mappe les résultats vers `ServingStatus` |

**La liste « do-not-depend-on » :** jamais un client de stockage, jamais `tonic`, jamais `service-runtime`.
Ajouter une telle arête réintroduirait le couplage que ce crate a été créé pour briser.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | `check()` est bon marché (un ping de joignabilité, pas une requête) | convention de contrat | une probe lourde fait flapper le service sous charge |
| I2 | Tout `Err` d'une seule probe rétrograde tout le service | boucle de readiness de `service-runtime` | service → `NOT_SERVING` jusqu'au prochain tick propre |
| I3 | Aucun état d'échec collant — la readiness se redérive à chaque tick | par conception (aucun état ici) | — |
| I4 | Une closure `FnProbe` doit être ré-appelable (`Fn`, pas `FnOnce`) | borne de type | erreur de compilation |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

N/A — crate de contrat pur, aucun flot de contrôle runtime propre. La boucle de sondage (premier tick immédiat,
écritures uniquement sur transition vers le reporter gRPC) vit dans `service-runtime` ; la vérification de
joignabilité réelle vit dans le `health::probe` de chaque crate de stockage. Ce crate n'est que le trait qui
les joint.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| crates de stockage | aval | Published Contract | `impl HealthProbe` sur leur client | le signal de readiness de chaque service |
| `service-runtime` | aval | Published Contract | sonde `Vec<Arc<dyn HealthProbe>>` | le mapping readiness → santé gRPC |

> **Seam de stabilité :** le trait `HealthProbe` est l'API publique entière ; un changement de signature se
> propage à chaque crate de stockage *et* au runtime simultanément — à traiter comme un changement cassant dur.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

N/A — pur. Il n'émet rien ; les événements `tracing` de readiness (`"health status changed"`) sont émis par
`service-runtime` quand il agit sur un résultat de probe.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Placer le trait de probe dans une feuille du graphe pour que stockage et runtime ne se couplent jamais | [`README §Architecture`](../README.md) | Accepted |
| Fail-to-`NOT_SERVING`, auto-effaçant (aucun état collant) | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — un contrat de commodité ; son levier est purement la forme du graphe de dépendances.
- **Stabilité :** contrat stable — le trait est stabilisé ; le changer est un changement cassant à l'échelle de la flotte.
- **Volatilité :** très faible — la surface est de deux items.
- **Capacités différées :** aucune ; une santé plus riche (dégradé vs down, dépendances pondérées) serait un
  nouvel enum sur le résultat, mais n'est pas modélisée aujourd'hui.
