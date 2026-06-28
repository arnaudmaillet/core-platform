---
i18n:
  source: ./DOMAIN.md
  source_sha256: 8d2ca3b63563fbed5a47381399fd8ff16bb5d75687a4b6a20d7a6a51931d4a23
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `infra-config` — Contrat de Domaine & Fonctionnel

> Configuration externalisée & hot-reload fail-closed : la couche IO/policy qui répond à *« avec quels chiffres tournent les crates de middleware pures, et comment changent-ils sans redémarrage ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Config infra externalisée — parser, valider, résoudre les bindings, et hot-reloader les `[section]`s que les crates pures ne doivent pas parser elles-mêmes |
> | **Couche** | `foundation` — la couche policy/IO alimentant les crates de middleware pures |
> | **Classe de sous-domaine** | **Supporting** — le plan de contrôle opérationnel de la policy à l'échelle de la flotte ; fort levier pendant les incidents |
> | **Abstraction(s) primaire(s)** | `InfraRegistry` + `Reloadable` (`infra_config::infra`, `infra_config::reload`) |
> | **Empreinte** | IO/avec état — possède l'IO fichier, le parsing TOML, un watcher `notify`, et les swaps `ArcSwap` |
> | **Posture en cas d'échec** | **fail-closed** — un document malformé ou invalide est rejeté ; la config saine précédente reste vivante |
> | **Dépend de** | `notify`, `toml`, `serde`, `arc-swap`, `tokio`, `resilience` + `traffic` (`serde`) |
> | **Consommé par** | `service-runtime` (charge + surveille) ; les services lisent les profils `[cache]`/`[resilience]` résolus |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `infra-config` fait autorité dans la flotte pour la **policy d'infrastructure externalisée** :
il répond à **« d'où viennent les timeouts/quotas/TTL/sampling, et comment se re-règlent-ils à chaud ? »** —
pour que les crates de mécanisme pures (`resilience`, `traffic`, les adaptateurs de cache, les dials télémétrie)
restent libres d'IO.

**Le problème difficile.** Les boutons critiques en incident (un timeout de circuit, un quota de débit, un
filtre de log) doivent changer *sans redéploiement*, mais les crates qui les *utilisent* doivent rester pures
et testables unitairement. `infra-config` absorbe toutes les parties dangereuses — IO fichier, parsing,
validation, sémantique d'inode-swap des ConfigMaps K8s, swap sans race — derrière un unique chemin de reload
fail-closed partagé par chaque section.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Posséder le *mécanisme* qu'une section configure (couches Tower, le limiteur, adaptateurs de cache) → ce
  sont les crates pures (`resilience`, `traffic`, …).
- ❌ Appliquer les dials télémétrie directement → il expose un `TelemetrySink` ; `service-runtime` fait le pont vers `telemetry`.
- ❌ Hot-reloader la *topologie* (quels profils/sections existent, quelle dépendance binde où) → figée au boot.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Section | Une catégorie d'infrastructure dans le document | `[resilience]`, `[cache]`, `[traffic]`, `[telemetry]` |
| Catalog | Profils nommés + une table de bindings, une forme par section | `Catalog<L>`, `catalog::validate_bindings` |
| Binding | Un nom de dépendance/namespace → un profil de classe-de-service | (résolu dans chaque `*Registry`) |
| Wire vs Runtime | Spec serde plate parsée du TOML vs handle `ArcSwap` lu par le chemin de données | `*ProfileSpec` vs `*Profile` |
| Registry | Le détenteur résolu et hot-reloadable d'une section | `InfraRegistry`, `ResilienceRegistry`, `CacheRegistry`, `TrafficRegistry`, `TelemetryRegistry` |
| Reloadable | La cible du watcher — parse + valide + swap | `Reloadable::reload` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `InfrastructureConfig` | document parsé | `from_toml` + `validate` ; les nouvelles sections sont `Option<…>` pour la compat ascendante |
| `InfraRegistry` | registry agrégé | Résout chaque section ; `apply` swap tout-ou-rien |
| `Reloadable` | trait (seam) | Découple le watcher de la forme de toute section ; `reload(raw)` est fail-closed |
| `Catalog<L>` | forme partagée | Un seul chemin de résolution/validation réutilisé par chaque section |
| `spawn_watcher` | fonction | Retourne un guard qui **doit rester vivant** pour que la surveillance continue |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- L'IO fichier, le parsing TOML, la validation fail-closed, les bindings de flotte, et le chemin de hot-reload
  basé sur `notify` — la *plomberie de policy* dont chaque crate de mécanisme pure doit rester libre.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Couches Tower / état de circuit-breaker | `resilience` | Le mécanisme est pur ; ce crate ne fournit que ses chiffres |
| Le limiteur GCRA | `traffic` | Même séparation de pureté |
| Le pipeline télémétrie | `telemetry` | Ce crate expose un `TelemetrySink` ; le pont vit dans `service-runtime` |

**La liste « do-not-depend-on » :** jamais `tonic`/`http` ni un crate de service. Il dépend *vers le haut* des
crates pures (`resilience`, `traffic`) uniquement pour leurs types wire `serde` — jamais leur runtime.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | Chaque section présente valide *avant* tout swap (tout-ou-rien) | `apply` / `Reloadable::reload` | `ConfigError::Validation` ; la config précédente reste vivante |
| I2 | Tous les swaps se font dans **une** tâche écrivain (pas de lecture déchirée/race) | l'unique watcher spawné | — |
| I3 | Un binding/`default_profile` doit référencer un profil défini | `catalog::validate_bindings` | `ConfigError::Validation` |
| I4 | Le watcher observe le **répertoire parent**, pas le chemin du fichier | `spawn_watcher` | (sinon l'inode-swap de ConfigMap K8s passe inaperçu) |
| I5 | La topologie (sections/profils/bindings) est figée au boot ; seuls les *contenus* hot-reloadent | câblage à la résolution | nécessite un redémarrage |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Boot.** `load_from_path` lit + parse `infrastructure.toml` ; `InfraRegistry::from_config` valide chaque
section et résout les bindings en handles runtime backés par `ArcSwap`. Un document malformé/invalide fait
échouer le boot — le pod ne sert jamais une mauvaise config.

**Boucle de hot-reload.** `spawn_watcher` surveille le *répertoire parent* (K8s échange l'inode du symlink
`..data`, donc une surveillance du chemin de fichier devient sourde après le premier changement). Un événement
`notify` → coalescence des rafales → relecture → `Reloadable::reload` : parse + valide **toutes** les sections
présentes, puis swap **toutes** via `ArcSwap` (fail-closed, tout-ou-rien). Le guard retourné par `spawn_watcher`
doit survivre au processus.

**Chemin de données.** Les consommateurs tiennent des handles runtime (`*Profile`) et font `ArcSwap::load` d'un
snapshot par opération — sans verrou, toujours cohérent au sein d'une seule décision.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `resilience` | amont | Conformist (types `serde`) | `ResilienceProfileSpec` | le parsing de `[resilience]` |
| `traffic` | amont | Conformist (types `serde`) | `TrafficProfileSpec` | le parsing de `[traffic]` |
| `service-runtime` | aval | Published Contract | `load_from_path`, `spawn_watcher`, `InfraRegistry` | le boot de flotte + hot-reload |
| `telemetry` | indirect | Separated Interface | `TelemetrySink` (ponté par `service-runtime`) | le re-réglage live log/sampling |

> **Seam de stabilité :** `InfraRegistry`, `Reloadable`, et les variantes de `ConfigError` sont le contrat
> public sur lequel `service-runtime` se construit.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| reload appliqué / rejeté | `tracing` | un document de config est swappé ou échoue la validation | dashboards ops pendant un push de config |
| surveillance fichier | effet de bord | un watcher `notify` sur le répertoire parent de la config | la couche inotify/FSEvents de l'OS |

Il ne mute aucun store externe ; ses seuls effets de bord sont la lecture du fichier de config et le swap des
handles `ArcSwap` en mémoire.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Séparation mécanisme pur vs IO/policy (le middleware ne lie ni `notify`/`toml`) | [`README §Architecture`](../README.md) | Accepted |
| Swap fail-closed, toutes-sections-ou-aucune dans une tâche écrivain unique | [`README §Architecture`](../README.md) | Accepted |
| Surveiller le répertoire parent pour survivre aux inode-swaps de ConfigMap K8s | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Supporting — le plan de contrôle de la policy à l'échelle de la flotte ; son levier est
  opérationnel (re-régler un incident sans redéploiement).
- **Stabilité :** en évolution — ajouter une nouvelle `[section]` est le chemin de croissance attendu (un spec
  + un type live + un registry, réutilisant `Catalog<L>`).
- **Volatilité :** moyenne — les *contenus* de section sont de la config ; la *machinerie de résolution* est stabilisée.
- **Capacités différées :** aucune structurelle ; chaque nouvelle préoccupation infra devient une nouvelle section.
