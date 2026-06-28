---
i18n:
  source: ./0011-media-byte-free-control-plane.md
  source_sha256: 5e0f0f69761fbf93909e6cefe38c176b72a5e3acd59b8fb823d023faa1cc5310
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0011-media-byte-free-control-plane.md`](./0011-media-byte-free-control-plane.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0011 : Media est un plan de contrôle sans-octets avec livraison fail-open et Screen CSAM fail-closed

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** media ; moderation ; post, profile, search
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Média signifie de gros binaires. Pousser des octets via gRPC ou Kafka ruine les deux (limites de
taille de message, pression sur le broker, mémoire). Pourtant le *cycle de vie* de l'asset —
uploadé → screené → transformé → ready — doit être suivi de façon autoritaire, ne doit **jamais**
laisser du contenu non-sûr (CSAM) atteindre `ready`, et doit continuer à servir même quand une
dépendance non-sécurité est dégradée.

## Décision

`media` est un **plan de contrôle sans-octets** : les octets vont **direct au store objet via URL
pré-signées**, jamais sur gRPC/Kafka ; media possède le **SoR métadonnées/cycle de vie** de l'asset
(Postgres + Redis), les octets étant dans S3/MinIO référencés par `StorageKey`. Il fait tourner trois
plans — (A) courtier d'upload, (B) pipeline de transformation Kafka async, (C) courtage
livraison/CDN — avec une **posture de défaillance par catégorie** : la résolution de livraison
**échoue ouverte**, tandis que le **`Screen` CSAM gardant la mise-en-ready échoue fermé** (un asset ne
peut atteindre `ready` sans le passer ; un takedown de modération met en quarantaine/supprime).

## Conséquences

- **Positives :** le plan octets ne stresse jamais le mesh ; le cycle de vie fait autorité ; le
  contenu non-sûr ne peut passer en ligne ; la livraison dégrade gracieusement.
- **Négatives / compromis accepté :** les clients doivent gérer le handshake d'upload pré-signé ; le
  Screen est une dépendance synchrone sur la mise-en-ready (bornée par un timeout dur).
- **Clôt :** la pression octets-sur-le-fil et le risque de contenu-non-sûr-en-ligne.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Streamer les octets via le service / Kafka | Limites de taille, pression broker, explosion mémoire |
| Stocker les octets dans la base | Mauvais outil ; gonfle le SoR ; pas de voie CDN |
| Screen fail-open | Laisse du CSAM atteindre `ready` pendant une panne — inacceptable |
