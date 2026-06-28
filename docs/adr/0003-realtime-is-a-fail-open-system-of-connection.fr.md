---
i18n:
  source: ./0003-realtime-is-a-fail-open-system-of-connection.md
  source_sha256: 9adb2602d75624e93fab1ea213407f623ffbce270e5eeedef27edeb32fec35cc
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0003-realtime-is-a-fail-open-system-of-connection.md`](./0003-realtime-is-a-fail-open-system-of-connection.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0003 : Realtime est un System-of-Connection fail-open, jamais un record store

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** realtime (nouveau contexte TIER-1) ; chat, notification, counter, post
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Le push live côté client (DMs, notifications, pics d'engagement) est déjà implémenté deux fois — `chat`
et `notification` streament chacun vers les clients à leur façon ad-hoc. Une troisième voie temps réel
sur-mesure aggraverait la duplication et la surface opérationnelle. Il nous faut **un seul plan de
livraison**. Deux pièges à éviter : le polling (un amplificateur de charge inversé — les clients
martèlent le mesh pour découvrir que rien n'a changé), et laisser le plan de livraison accumuler une
durabilité qu'il ne devrait pas posséder (auquel point une panne perd des données au lieu d'être
récupérable).

## Décision

Nous construisons **`realtime` comme un System-of-Connection / Delivery TIER-1, fail-open** — un
bulkhead devant le mesh gRPC qui **ne stocke jamais d'enregistrements** : s'il disparaît, les clients
re-synchronisent depuis les SoR propriétaires. Règles constituantes :

1. **Une socket multiplexée par appareil.** WSS sur `:443` portant une enveloppe
   `{stream_seq, channel, ack_required, payload}` — **pas** gRPC-vers-client, **pas** SSE. Une socket
   remplace les streams client par-feature.
2. **Fan-out ciblé.** Lookup de registry + livraison ciblée, pas broadcast-and-filter.
3. **Fail-open avec durabilité déléguée.** Si un destinataire est offline, déléguer à `notification`
   (APNs/FCM) ; à la reconnexion le client re-synchronise depuis les SoR via un token de séquence. Une
   livraison en vol perdue n'est jamais une donnée perdue.
4. **Listeners séparés.** gRPC interne sur `:50066` (gateway) et `:50067` (dispatcher) ; le WebSocket
   public est un listener *séparé* — brisant délibérément la convention un-port de la flotte. Deux
   binaires : `realtime-gateway` (edge stateful) et `realtime-dispatcher` (worker stateless).

## Conséquences

- **Positives :** une socket multiplexée par appareil effondre les silos chat/notification ; une panne
  ne peut perdre de données car realtime n'en possède aucune — la récupération est une re-sync ; la
  charge de connexion est isolée du mesh gRPC.
- **Négatives / compromis accepté :** un edge stateful avec des préoccupations C10M (conns inactives en
  futures parkées, SLO mémoire dur par-connexion, file de shed bornée, reaping par heartbeat) ; il
  brise intentionnellement la convention un-port, que le déploiement doit accommoder.
- **Clôt :** le troisième silo temps réel ad-hoc, et la voie d'amplification de charge du polling.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Polling client | Amplificateur de charge inversé — la plupart des polls ne découvrent rien, martelant le mesh |
| gRPC-streaming-vers-client / SSE | Pas de multiplexage ni d'enveloppe côté client ; recréerait des streams par-feature |
| Fan-out broadcast-and-filter | Gaspilleur à l'échelle ; chaque node traite chaque message pour en jeter la plupart |
| Faire de realtime un record store | Force des garanties de durabilité qu'il ne devrait pas posséder ; défait le modèle de récupération fail-open |
