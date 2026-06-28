---
i18n:
  source: ./DOMAIN.md
  source_sha256: 7d2d402ecc06cf7dc45753cafa8e2e1bf5c96f01369888d517003dba8d4e1722
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `realtime` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Live Delivery — le System-of-Connection côté client |
> | **Classe de sous-domaine** | **Supporting** — il accélère la livraison mais ne possède aucune valeur ; les SoR (`chat`/`notification`/`counter`) possèdent chaque octet qu'il relaie. Sur-mesure (pas Generic) car l'edge C10M est fait main |
> | **System of …** | **Connection / Delivery** — explicitement **jamais** un System of Record |
> | **Racine(s) d'agrégat** | `Connection` (`domain::connection`), avec `Session`, `SubscriptionSet`, `SequenceState` |
> | **Tier** | **TIER-1** |
> | **Posture de défaillance** | **Fail-open** — un raté de livraison coûte de la latence, jamais des données ; les clients re-synchronisent depuis les SoR à la reconnexion |
> | **Contextes amont** | clients utilisateur via WSS (pas les services internes) ; `auth` via `auth-context` (une vérification, pas un appel) |
> | **Contextes aval** | aucun de référence ; délègue la livraison offline à `notification` (APNs/FCM) |
> | **Journal de décisions** | [`ADR-0003`](../../../../docs/adr/0003-realtime-is-a-fail-open-system-of-connection.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `realtime` est l'autorité pour **la livraison live côté client** : il répond à
**« quels appareils de cet utilisateur sont connectés maintenant, et comment amener cet événement déjà
durable sur l'appareil exact à l'instant où il arrive ? »**

**Le problème difficile** est double. **Le piège de charge inversée du polling :** à très grande
échelle, des clients qui pollent l'edge couplent le QPS cœur au nombre d'utilisateurs *inactifs* —
chaque poll vide paye TLS + auth + fan-out complets pour ne rien livrer, un DDoS auto-infligé sur le
mesh. **La prolifération de connexions :** `chat` et `notification` ont chacun développé leur propre
streaming client, donc un appareil tient plusieurs sockets avec des heartbeats redondants. Realtime
inverse le premier (le travail suit les *événements*, pas les yeux) et réduit le second à une seule
socket multiplexée par appareil.

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Stocker une entité ou un message → la durabilité vit dans `chat`/`notification`/`counter`.
- ❌ Se placer sur un write path synchrone → il n'est jamais sur la voie d'envoi d'un message ou de persistance d'une notification.
- ❌ Ré-authentifier par frame → l'edge token est vérifié une fois au handshake.
- ❌ Écrire l'autorisation niveau-contenu → faite en amont au moment de l'émission ; realtime ne vérifie que la propriété de scope de canal.

---

## 2. Langage Omniprésent

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Connection | Une socket client live, multiplexée, et son cycle de vie | `Connection`, `ConnectionState` |
| Session | L'identité authentifiée liée à une connexion au handshake | `Session` |
| Channel | Un flux logique multiplexé sur l'unique socket (`dm`, `notif`, `presence`, `counter:<id>`, `feed:<id>`) | `ChannelRef`, `ChannelKey`, `ChannelClass` |
| Subscription set | Les canaux auxquels une connexion est abonnée (plafonnés) | `SubscriptionSet` |
| Stream sequence | Le token monotone par-flux que les clients dédupliquent et re-synchronisent | `StreamSeq`, `SequenceState` |
| Presence | Vivacité interne (online/offline) dérivée des connexions — pas un stream produit | `PresenceState` |
| Targeted vs broadcast | Un fan-out qui nomme un *utilisateur destinataire* vs un qui nomme une *entité* | (modes du dispatcher) |
| Node / registry | Une instance gateway et la carte de routage `user → node` | port `ConnectionRegistry` |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `Connection` | racine d'agrégat | Cycle de vie + la discipline de file d'envoi bornée / shed pour une socket |
| `Session` | entité | L'identité épinglée authentifiée au handshake |
| `SubscriptionSet` | VO | Abonnements de canaux, plafonnés (`RTM-3002`) et scope-vérifiés |
| `SequenceState` / `StreamSeq` | VO | Séquencement monotone par-flux pour dédup + re-sync côté client |
| `ChannelRef` / `ChannelKey` / `ChannelClass` | VO | L'identité d'un canal et son scope de propriété |
| `PresenceState` / `ConnectionState` / `CloseReason` / `DeliveryGuarantee` | enum | Le vocabulaire fermé cycle-de-vie + livraison |

**Cycle de vie de connexion :**

```
handshake (verify token → bind Session) --> Active --(subscribe within scope)--> delivering
     │ heartbeat timeout / shed / drain                                              │
     ▼                                                                               ▼
   Closed  ◄──────────────── reconnect (jittered backoff) ◄──── drain (control frame) ┘
```

> **Transitions légales uniquement.** Une connexion ne peut s'abonner qu'aux canaux scoped à son
> identité épinglée (Alice → `dm:alice`, jamais `dm:bob`) — sinon `RTM-3001`. Un consommateur lent est
> **shed** (`RTM-5001`), jamais bufferisé sans borne.

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité pour :**
- Rien de durable. Il ne possède que l'**état de routage Redis éphémère** — `presence:{user_id}` → node(s) et le tissu Pub/Sub de saut-de-node. Tout est TTL'd et auto-réparant.

**Tout ce qu'il relaie est possédé ailleurs :**

| Donnée relayée | Possédée par | Atteint realtime via | Durabilité |
|---|---|---|---|
| Notifications / badges | `notification` | `notification.v1.events` (targeted) | `notification` le persiste |
| Magnitudes d'engagement | `counter` | `counter.v1.popularity` (broadcast) | `counter` le détient |
| Cycle de vie des posts | `post` | `post.v1.events` (broadcast) | `post` le persiste |
| Messages | `chat` | (non consommé — chat fait tourner son propre plan live, coexist-first) | `chat` le persiste |

**La liste « ne-pas-écrire » :** realtime n'écrit aucun SoR ; un miss de registry (destinataire
offline) est un no-op, la voie téléphone-verrouillé étant déléguée à `notification`.

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | Le plan ne stocke rien de référence ; un miss de registry est un no-op | domaine + application | (par conception) |
| I2 | Authentifier une fois, au handshake ; jamais ré-vérifier par frame | frontière infrastructure | `RTM-1001` (handshake) |
| I3 | La seule autorisation est la propriété de scope de canal face à l'identité épinglée | domaine | `RTM-3001` |
| I4 | Un consommateur lent est shed, pas bufferisé (file par-connexion bornée) | infrastructure | `RTM-5001` |
| I5 | Fail-open — un événement live perdu n'est jamais un message perdu ; le client re-sync via `StreamSeq` | application | (récupération, pas erreur) |
| I6 | Les abonnements sont plafonnés par connexion | domaine | `RTM-3002` |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**Connexion (handshake → livraison).**
1. Le client se connecte via WSS (`:8443`, derrière un LB L4) ; l'edge token est vérifié une fois via `auth-context` → une `Session` est liée.
2. La gateway écrit l'entrée de registry `user → node` et s'abonne à son canal de node.
3. Le client s'abonne aux canaux dans le scope de son identité ; les frames ne sont pas ré-authentifiées ensuite.

**Le pont interne→externe (trois étapes).**
1. **Émettre une fois :** un service cœur publie vers Kafka — aucun nouvel appel synchrone vers l'intérieur.
2. **Résoudre :** `realtime-dispatcher` (sous `run_consumer`) résout le destinataire face au registry (targeted) ou nomme un canal de broadcast (entité).
3. **Dernier saut :** publier vers le canal Redis du node propriétaire → la gateway le remet à la mailbox bornée de la connexion → écriture socket. Cœur→appareil = un saut Kafka + un saut Redis + une écriture socket.
- **Idempotence :** le fan-out est naturellement idempotent ; les frames live dupliquées sont inoffensives (le client déduplique sur `StreamSeq`) ; un événement non-routable (`RTM-8002`/`RTM-8003`) se replie en `Ok`.

**Deux modes de fan-out.** *Targeted* (notification/DM/presence) nomme un destinataire → résolution
registry → saut vers le(s) node(s) propriétaire(s). *Broadcast* (counter/feed) nomme une entité →
publier une fois vers le canal de broadcast de la flotte → chaque node livre à ses abonnés locaux.

**Dégradation & reaping.** Les connexions semi-ouvertes sont reapées au timeout de heartbeat
(`RTM-5002`, libère FD + slot de registry, bascule la présence offline). Au drain de node,
stop-accept → frame de contrôle de reconnexion avec backoff jitter (`RTM-5003`) → drain.

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| clients utilisateur | amont | OHS (Published Language) | l'enveloppe multiplexée WSS `{stream_seq, channel, ack_required, payload}` | un changement d'enveloppe casse chaque client |
| `auth` | dépendance | Conformist (verify-only) | vérif edge-token ES256 via `auth-context` | les nouveaux handshakes échouent si le format de token change |
| `notification` | amont + délégué | ACL | consomme `notification.v1.events` ; délègue le push offline | le décodage casse / la voie offline perdue |
| `counter` | amont | ACL | consomme `counter.v1.popularity` | le décodage broadcast casse |
| `post` | amont | ACL | consomme `post.v1.events` | le décodage broadcast casse |
| `chat` | pair | Separate Ways (coexist) | non consommé — chat possède son propre plan live | — (la consolidation est une décision future) |

> **Anti-Corruption Layer :** `infrastructure/decode.rs` mappe chaque événement wire amont vers le
> `DeliverableEvent` interne ; la charge utile opaque est relayée, jamais interprétée.

---

## 8. Événements de Domaine (sémantique, pas wire)

> Realtime **ne publie rien de référence**. La présence, si exposée, est de la vivacité interne
> seulement — pas un stream System-of-Record.

| Événement | Signifie | Émis quand | Qui réagit |
|---|---|---|---|
| — (aucun de référence) | realtime n'affirme aucun fait métier durable | — | — |

Il consomme les faits de `notification`/`counter`/`post` ; leurs sens sont possédés par le §8 des
Domain Cards de ces contextes et consolidés dans `docs/domain/EVENT_CATALOG.md`.

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| Realtime est un System-of-Connection fail-open, jamais un record store | [`ADR-0003`](../../../../docs/adr/0003-realtime-is-a-fail-open-system-of-connection.md) | Accepté |
| Coexist-first avec le plan live de `chat` (ne pas consommer `chat.message.sent` pour l'instant) | _voir conséquences ADR-0003_ | Accepté |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Supporting — il fait *sentir* le produit live mais ne possède aucune vérité ; la valeur vit dans les SoR. L'investissement va à l'edge C10M sur-mesure, pas à la propriété des données.
- **Volatilité :** faible-à-moyenne — les nouvelles sources de fan-out (canaux) sont additives ; le modèle de connexion et l'enveloppe sont stables.
- **Dette de modélisation connue :** le câblage runtime `SIGTERM` → `broadcast_drain` n'est pas encore connecté ; la variante de backpressure `NodeChannel` en gRPC interne n'est pas construite (le tissu Redis Pub/Sub est la v1).
- **Capacités différées :** voie de transport WebTransport/HTTP-3 ; présence comme stream produit ; routage cross-région / multi-PoP ; consolidation du streaming client `chat` + `notification` (la couture — les réduire à des producteurs d'événements — est laissée ouverte).
