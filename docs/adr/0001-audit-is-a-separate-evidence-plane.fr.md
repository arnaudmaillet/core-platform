---
i18n:
  source: ./0001-audit-is-a-separate-evidence-plane.md
  source_sha256: 2e86ba3dc484c255b8542daf27ded5201d60f958e46aa2fa3a46715ba263fc0d
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0001-audit-is-a-separate-evidence-plane.md`](./0001-audit-is-a-separate-evidence-plane.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0001 : Audit est un plan de preuve infalsifiable séparé, pas un agrégateur de logs

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** audit (nouveau contexte TIER-0) ; producteurs moderation, auth, account
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Il nous faut un enregistrement faisant autorité de **qui a fait quoi, à qui, quand et sous quelle
autorité** — pour SOC2, le DSA et la responsabilité RGPD (Art. 5(2)). L'instinct est de le dériver de
la télémétrie (logs/traces). Cet instinct est faux, et démontrablement : **la télémétrie est mutable,
échantillonnée et à rétention plafonnée, alors que la preuve doit être infalsifiable, complète et
non-répudiable.**

Pire, confondre les deux fabrique une contradiction juridique directe. Le RGPD Art. 17 (droit à
l'effacement) exige de pouvoir supprimer les données d'un sujet ; l'Art. 5(2) (responsabilité) exige
de pouvoir prouver ce qui leur est arrivé. Si la preuve *est* la donnée, satisfaire l'un viole
l'autre. Un agrégateur de logs ou des tables d'audit par-service ne peuvent résoudre cela — ils n'ont
pas de chaîne globale, et supprimer une ligne pour honorer l'effacement détruit silencieusement
l'enregistrement de responsabilité.

## Décision

Nous construisons **`audit` comme un plan de preuve dédié, TIER-0, append-only et chaîné par hash** —
un puits terminal qui *enregistre les décisions et n'agit jamais*, distinct du plan de télémétrie.
Règles constituantes :

1. **Immuabilité = quatre domaines de confiance indépendants à forger.** Une chaîne de hash à 3
   couches sur les événements, stockée INSERT-only en Postgres **et** répliquée en WORM Object-Lock,
   avec un checkpoint Merkle signé ancré à un témoin externe. Forger l'histoire requiert de
   compromettre les quatre.
2. **RtbF RGPD par crypto-shred, pas par suppression.** La PII de chaque sujet est scellée sous un DEK
   par-sujet avec un pseudonyme ; la chaîne hashe le *ciphertext*. L'effacement = détruire la clé —
   l'enregistrement et sa preuve survivent, la PII devient irrécupérable. Le legal-hold prime sur
   l'effacement.
3. **Ingestion dual-lane.** Kafka async (`run_consumer`, producteur **fail-open**) porte ~99% du
   volume et absorbe les pics ; une voie gRPC synchrone `RecordPrivileged` est **fail-closed** — une
   action de break-glass est *refusée* si elle ne peut être enregistrée d'abord.
4. **Partitionnement hybride** par tenant + catégorie, avec le sujet indexé pour les lookups
   d'effacement.
5. Réalisé en deux binaires : `audit-server` (`:50068`) et `audit-worker` (`:50069`) ; namespace
   d'erreur `AUD-XXXX`.

## Conséquences

- **Positives :** l'effacement (Art. 17) et la responsabilité (Art. 5(2)) coexistent sans
  contradiction ; la falsification requiert de percer quatre domaines de confiance ; une action de
  break-glass ne peut jamais procéder non enregistrée ; les exigences de preuve SOC2/DSA sont
  satisfaites par construction.
- **Négatives / compromis accepté :** un nouveau service TIER-0 à opérer ; la voie synchrone ajoute
  une dépendance dure sur le chemin break-glass ; la custody des clés en prod (KMS/HSM) et le témoin
  externe (RFC3161 / WORM cross-compte) sont différés au provisionnement IAM/org ; les consumers
  crypto-shred et de balayage de rétention attendent des sources amont.
- **Clôt :** la contradiction RGPD Art. 17 ⇄ Art. 5(2), et la lacune probatoire SOC2/DSA.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Dériver l'audit des logs / d'un SIEM | Mutable, échantillonné, rétention plafonnée — ne peut prouver la non-répudiation ni la complétude |
| Tables d'audit par-service | Pas de chaîne globale ; honorer l'effacement en supprimant des lignes détruit l'enregistrement de responsabilité |
| Stocker la PII en clair, supprimer sur demande | La suppression casse la chaîne de hash ; effacement et preuve deviennent mutuellement exclusifs |
