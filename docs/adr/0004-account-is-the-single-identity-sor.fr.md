---
i18n:
  source: ./0004-account-is-the-single-identity-sor.md
  source_sha256: 1ef35508a594e6edb0e41906636fe93904220ce6ef470dfac81e7fcfcb76eaa2
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0004-account-is-the-single-identity-sor.md`](./0004-account-is-the-single-identity-sor.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0004 : Account est le SoR d'identité unique et possède une voie d'événements sortante

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** account ; consommateurs audit, profile
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Une identité réelle (email, téléphone, KYC, rôles) doit vivre en un seul endroit, sinon l'effacement
RGPD devient impossible — chaque copie fantôme est un enregistrement incontrôlé que l'Art. 17 ne peut
atteindre. Account était aussi un **producteur fantôme** : il possédait l'identité mais ne publiait
rien, donc les contextes aval n'avaient aucun moyen de réagir aux changements de cycle de vie hormis
en allant chercher dans le store d'account.

## Décision

Account **est le système de référence unique du compte utilisateur** — existence, métadonnées
d'identifiants, KYC, rôles et état RGPD — et **possède une voie d'événements sortante**
(`account.v1.events`, publiée après chaque save durable). Les contextes aval (`audit`, `profile`)
consomment des références ; aucun ne détient d'identité faisant autorité. Un `gdpr_deletion_requested`
propage l'effacement (audit crypto-shred le sujet), fermant la boucle Art. 17 de bout en bout.

## Conséquences

- **Positives :** l'identité a un seul foyer ; l'effacement RGPD a un unique déclencheur faisant
  autorité qui se propage ; l'aval reste découplé (événements, pas accès direct au store).
- **Négatives / compromis accepté :** account doit garantir l'émission d'événement après save (une
  discipline outbox/publish-après-save), ajoutant une responsabilité de write path.
- **Clôt :** la lacune du producteur fantôme et le problème d'effacement de l'identité fantôme.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Laisser les services lire le store d'account directement | Couple chaque consommateur au schéma d'account ; pas de propagation d'effacement |
| Répliquer l'identité dans chaque service | Multiplie les copies de PII ; rend l'effacement Art. 17 inapplicable |
