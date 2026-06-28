---
i18n:
  source: ./DOMAIN.md
  source_sha256: cc401a87035497686aeb395a03b0ced190a39a52cff4f979e88157065cfb6db4
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `audit` — Contrat de Domaine & Fonctionnel

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Compliance Evidence — la piste d'audit infalsifiable |
> | **Classe de sous-domaine** | **Supporting** — légalement non négociable mais pas un différenciateur visible ; sur-mesure (pas Generic) car aucun outil SIEM/audit sur étagère ne réconcilie l'effacement RGPD Art. 17 avec la responsabilité Art. 5(2) |
> | **System of …** | **Record / Evidence** pour « qui a fait quoi, à qui, quand, sous quelle autorité, avec quel résultat » |
> | **Racine(s) d'agrégat** | `AuditRecord` (`domain::record`), avec la chaîne par-partition `ChainLink` / `ChainHead` (`domain::chain`) et `MerkleCheckpoint` (`domain::checkpoint`) |
> | **Tier** | **TIER-0** |
> | **Posture de défaillance** | **Mixte** — *fail-open* chez les producteurs (la vivacité d'audit ne fait jamais brownout du mesh) ; *fail-closed* sur la durabilité et sur la voie synchrone de break-glass |
> | **Contextes amont** | `moderation`, `auth`, `account` via **Conformist + ACL** (audit consomme leurs streams et traduit chaque forme wire étrangère en `AuditEvent` dans `infrastructure/decode.rs`) |
> | **Contextes aval** | aucun de référence — audit est un **puits terminal** ; il ne publie rien que d'autres consomment |
> | **Journal de décisions** | [`ADR-0001`](../../../../docs/adr/0001-audit-is-a-separate-evidence-plane.md) |

---

## 1. Capacité Métier & Non-Objectifs

**Capacité.** `audit` est l'autorité pour **la preuve de conformité** : il répond à
**« peut-on prouver que cet enregistrement de qui-a-fait-quoi est complet et n'a jamais été altéré, et
le détecterions-nous s'il l'était ? »**

**Le problème difficile.** La télémétrie et une piste d'audit se ressemblent à l'écran mais sont des
substances différentes : la télémétrie est best-effort, mutable, échantillonnée et à rétention
cyclique ; la preuve doit être zéro-perte, append-only, infalsifiable, complète, conservée des
années, et effaçable au niveau *champ*. Les confondre place la PII dans des index de logs
incontrôlés et fait qu'une demande d'effacement RGPD Art. 17 contredit directement le devoir de
responsabilité Art. 5(2).

**Non-objectifs — ce que ce contexte ne fait délibérément PAS :**
- ❌ Le logging applicatif général / l'observabilité → possédés par le plan de télémétrie (crate `telemetry`).
- ❌ Prendre des décisions ou agir — audit *enregistre* les décisions des autres et n'agit sur aucune.
- ❌ Détenir le mapping identité↔pseudonyme → possédé par `account` ; audit ne voit que des pseudonymes.
- ❌ Publier des événements de référence → c'est un puits terminal.

---

## 2. Langage Omniprésent

> Les termes inter-contexte (subject, lawful basis) vivent dans `docs/domain/UBIQUITOUS_LANGUAGE.md`.

| Terme | Sens dans ce contexte | Symbole de code |
|---|---|---|
| Audit record | Une entrée immuable, chaînée par hash, du registre de preuve | `AuditRecord` |
| Chain link / head | Le lien `H(prev ‖ canonical(payload) ‖ sequence_no)` et la tête courante d'une chaîne par-partition | `ChainLink`, `ChainHead` |
| Crypto-shred | Effacement par destruction de la clé d'un sujet, laissant l'enregistrement + la preuve intacts | via `PiiEnvelope` + `KeyVault` |
| PII envelope | Un ciphertext crypto-shreddable par-sujet ; la chaîne hashe le *ciphertext*, jamais le clair | `PiiEnvelope` |
| Checkpoint | Une racine de Merkle signée sur toutes les têtes de partition, ancrée à un témoin externe | `MerkleCheckpoint` |
| Legal hold | Une dérogation de rétention licite (Art. 17(3)) qui suspend l'effacement | `LegalHold` |
| Privileged action | Une action must-record-before-permitted enregistrée sur la voie synchrone fail-closed | `PrivilegedActionType` |
| Partition | Le shard `(tenant, category)` auquel appartient la chaîne d'un enregistrement ; le sujet est indexé, pas la clé de partition | `EventCategory` |

---

## 3. Modèle de Domaine

| Élément | Type | Frontière d'invariant gardée |
|---|---|---|
| `AuditRecord` | racine d'agrégat | Un enregistrement, une fois commité, est immuable et chaîné par hash à son prédécesseur |
| `NewAuditEvent` / `AuditEvent` | entité / VO | Un événement d'admission bien formé et dédupliqué avant/après son séquencement dans la chaîne |
| `ChainLink` / `ChainHead` | VO | Lien append-only par-partition ; `sequence_no` strictement monotone (un trou = signal de troncature) |
| `MerkleCheckpoint` | VO | Une racine signée liant toutes les têtes de partition à un instant donné |
| `PiiEnvelope` | VO | La PII n'existe que comme ciphertext crypto-shreddable ; le clair n'entre jamais dans la chaîne |
| `Actor` / `ResourceRef` | VO | Le qui pseudonyme et le sur-quoi on a agi |
| `LegalHold` / `RetentionPolicy` | VO | Quand l'effacement est permis vs suspendu licitement |
| `EventCategory` / `ActorType` / `Outcome` / `LawfulBasis` | enum | Le vocabulaire de classification fermé |

**Cycle de vie d'un enregistrement :**

```
intake --(dedupe: UUIDv5)--> sequenced (chain-linked) --(persist+archive)--> committed --(checkpoint)--> witnessed
                                                                                  └--(erase: destroy DEK)--> pii-shredded (chain still verifies)
```

> **Transitions légales uniquement.** Un enregistrement commité n'est jamais mis à jour ni supprimé
> (`UPDATE`/`DELETE` révoqués au niveau rôle DB). Une non-concordance de hash ou un trou de séquence
> à la lecture n'est pas un état récupérable — il lève `AUD-2001` / `AUD-2002` comme alarme de
> falsification.

---

## 4. Propriété des Données & Frontières

**Ce contexte est la source de vérité pour :**
- Le registre de conformité chaîné par hash — **Postgres** append-only (canonique) répliqué vers
  **Object-Lock WORM** (S3/MinIO, mode compliance). Aucun autre service ne l'écrit.
- Les DEK par-sujet et l'enregistrement de custody de la clé de signature (dans KMS/HSM, un domaine
  de confiance séparé).

**Ce contexte détient des copies qu'il ne possède PAS (read-model / dénormalisation) :**

| Donnée copiée | Possédée par | Maintenue fraîche via | Tolérance d'obsolescence |
|---|---|---|---|
| Faits de décision de modération | `moderation` | `moderation.v1.events` (`decision_recorded`, `enforcement_applied`) | cohérence à terme (lag Kafka) |
| Cycle de vie de session auth | `auth` | `auth.v1.events` (`session_issued`/`session_revoked`) | cohérence à terme |
| Cycle de vie compte + événements RGPD | `account` | `account.v1.events` | cohérence à terme |

**La liste « ne-pas-écrire » :** audit ne mute jamais l'état amont, ne résout jamais un pseudonyme en
identité réelle (ce mapping vit dans `account`), et n'émet jamais d'enregistrement dont d'autres
services dépendent.

---

## 5. Invariants & Règles Métier

| # | Invariant | Imposé à | En cas de violation |
|---|---|---|---|
| I1 | La PII n'entre jamais en clair dans la chaîne (sujets = pseudonymes ; PII = ciphertext `PiiEnvelope`) | domaine + infrastructure | (empêché par construction) |
| I2 | L'effacement est destruction de clé, pas suppression d'enregistrement — enregistrement, séquence, hash et métadonnées non-PII survivent | application | `AUD-5xxx` |
| I3 | Un sujet sous legal hold actif n'est pas shreddé (Art. 17(3) prime) | domaine | `AUD-5002` |
| I4 | Les actions les plus dangereuses échouent fermées — `RecordPrivileged` refuse sauf si enregistré durablement d'abord | application | `AUD-4004` |
| I5 | Les lectures sont aussi des preuves — chaque query/export est lui-même enregistré ; accès need-to-know | application | `AUD-3001`/`AUD-3002` |
| I6 | La chaîne est append-only et complète ; toute mutation/trou est détectable | domaine + rôle DB (`UPDATE`/`DELETE` révoqués) | `AUD-2001`/`AUD-2002` |
| I7 | Un checkpoint signé doit se réconcilier avec le témoin externe | application | `AUD-2004` |

---

## 6. Workflows & Orchestration

> En ligne jusqu'à ce qu'un C4 corrigé soit régénéré depuis `docs/domain/`.

**Ingestion en masse (async, fail-open).**
1. Un service de la flotte émet un événement de conformité vers Kafka (`audit.v1.events` ou un stream de décision) — fire-and-forget.
2. `audit-worker` consomme sous `run_consumer` ; déduplique par UUIDv5 (`AUD-1004` → `Ok`).
3. Décode le wire étranger → `AuditEvent` ; scelle toute PII dans une `PiiEnvelope`.
4. Ajoute à la chaîne par-partition → persiste vers Postgres → réplique vers WORM.
- **Discipline de commit :** l'offset Kafka n'avance **qu'après** persist durable + chaînage. Aucun offset commité ne dépasse un événement non persisté → zéro perte.
- **Idempotence :** UUIDv5 déterministe ; redelivery dédupliquée, donc chaque événement logique apparaît une fois.

**Action privilégiée (sync, fail-closed).**
1. Un appelant privilégié invoque `RecordPrivileged` sur `:50068`.
2. Audit tente un commit durable+chaîné dans un délai dur.
3. Succès → permet ; échéance/durabilité ratée → **refuse** (`AUD-4004`). L'action ne doit pas procéder non enregistrée.

**Effacement (crypto-shred).** Une demande RGPD Art. 17 détruit le DEK du sujet (sauf sous legal hold) → toute la PII scellée de ce sujet devient définitivement indéchiffrable alors que la chaîne se vérifie toujours. *(La boucle worker qui le pilote attend une source de demandes d'effacement ; le handler existe et est testé.)*

**Vérification d'intégrité.** Une boucle worker signe une racine de Merkle sur les têtes de partition, l'ancre au témoin externe, et un vérificateur autonome recalcule la chaîne et compare (`AUD-2004` en cas de divergence).

---

## 7. Relations de Contexte (extrait de Context-Map)

| Contexte voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `moderation` | amont | Conformist + ACL | `moderation.v1.events` | un changement de schéma de `decision_recorded` casse la chaîne de justification DSA scellée |
| `auth` | amont | Conformist + ACL | `auth.v1.events` | trous de preuve du cycle de vie des sessions |
| `account` | amont | Conformist + ACL | `account.v1.events` | le scellement PII + le déclencheur crypto-shred Art. 17 cassent |
| `account` | dépendance | Customer/Supplier | le mapping identité↔pseudonyme reste dans `account` | audit ne peut (et ne doit) pas résoudre les sujets |
| témoin externe / KMS | dépendance | domaine de confiance séparé | RFC 3161 TSA / WORM cross-compte ; KMS SigV4 | la garantie anti-falsification niveau opérateur s'affaiblit |

> **Anti-Corruption Layer :** `infrastructure/decode.rs` mappe chaque forme wire d'événement étranger
> (et le JSON `AuditEventWire` possédé par audit) vers le domaine `AuditEvent` — la dérive de schéma
> amont s'arrête à cette frontière.

---

## 8. Événements de Domaine (sémantique, pas wire)

> Audit **ne publie rien de référence** — c'est un puits terminal. Une alarme d'intégrité sur
> falsification/trou/divergence-de-témoin est un signal opérationnel, pas un stream System-of-Record.

| Événement | Signifie | Émis quand | Qui réagit |
|---|---|---|---|
| — (aucun) | audit n'affirme aucun fait métier vers l'extérieur | — | — |

Il **consomme** les faits de conformité de `moderation`/`auth`/`account` — leurs sens sont possédés
par le §8 des Domain Cards de ces contextes et consolidés dans `docs/domain/EVENT_CATALOG.md`.

---

## 9. Décisions & Justification

| Décision | ADR | Statut |
|---|---|---|
| Audit est un plan de preuve infalsifiable séparé, pas un agrégateur de logs | [`ADR-0001`](../../../../docs/adr/0001-audit-is-a-separate-evidence-plane.md) | Accepté |
| _(candidat à scinder)_ mécanisme crypto-shred RtbF ; split dual-lane fail-open/fail-closed ; schéma d'immuabilité hash-chain + WORM + témoin externe | _à rédiger_ | — |

---

## 10. Classification de Sous-domaine & Évolution

- **Classification :** Supporting — investir assez pour être juridiquement blindé et infalsifiable ; ne pas sur-dimensionner au-delà du besoin réglementaire.
- **Volatilité :** faible. Le modèle chaîne/shred/checkpoint est stable ; le changement est piloté par de nouvelles obligations *réglementaires* (nouvelle catégorie d'événement, nouvelle base licite), pas par le churn de features.
- **Dette de modélisation connue :** le consumer crypto-shred et le balayage d'expiration de rétention existent et sont testés, mais leurs boucles worker pilotes attendent des sources d'entrée (un stream de demandes d'effacement ; des politiques de rétention résolues).
- **Capacités différées :** authz de lecture + auto-audit des lectures via l'intercepteur d'ingress `auth-context` ; génération de rapports de transparence DSA ; réplication cross-région du registre. Le *provisionnement* KMS/témoin est un engagement IAM/org (le code est en place derrière les ports).
