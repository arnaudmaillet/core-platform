---
i18n:
  source: ./0002-moderation-decision-enforcement-sor-with-fail-closed-screen.md
  source_sha256: 926eb1be3327585cf54a190f2aa402ed4ee6d1b3cbd833c8f5a1e35edf516fd2
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0002-moderation-decision-enforcement-sor-with-fail-closed-screen.md`](./0002-moderation-decision-enforcement-sor-with-fail-closed-screen.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0002 : Moderation est le SoR décision/enforcement avec une porte Screen étroite fail-closed

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** moderation (nouveau contexte TIER-0) ; producteurs de contenu ; audit (consommateur)
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

La logique de confiance & sécurité se disperserait aujourd'hui dans chaque service de contenu —
chacun ré-implémentant des vérifications « est-ce actionné ? » sans réponse faisant autorité. Il nous
faut un unique **cerveau et système de référence pour les décisions d'intégrité et l'enforcement**. La
tension : un tel service doit pouvoir *bloquer* le contenu nuisible de façon autoritaire, mais
l'essentiel de son travail (ingérer des signaux, enregistrer des décisions) ne doit pas siéger sur le
write path synchrone comme passif de latence ou de disponibilité. Un service fail-open partout laisse
passer du contenu nuisible pendant une panne ; un fail-closed partout rend toute publication otage de
sa disponibilité.

Moderation doit aussi être soigneusement cadré : ce n'est **pas** un classifieur (cela couplerait la
politique au cycle de vie ML), **pas** un store de contenu, et **pas** une UI de revue.

## Décision

Nous construisons **`moderation` comme le SoR décision/enforcement TIER-0** exposant une **interface
à trois plans**, chaque plan avec la posture de défaillance qu'exige sa sémantique :

1. **Ingestion async** — signaux/signalements affluent via Kafka (`run_consumer`, fail-open). C'est la
   voie de masse et elle ne bloque jamais les producteurs.
2. **Enforcement hot-read** — « cette entité est-elle actionnée ? » servi depuis une voie de lecture
   rapide, fail-open (absence d'enregistrement ≠ bloqué).
3. **Porte `Screen` étroite fail-closed** — une unique vérification synchrone de pré-publication pour
   les catégories screenées. Si moderation est indisponible, `Screen` **refuse** — le contenu n'est
   pas publié.

Moderation enregistre chaque décision une fois et émet un événement de preuve dédié
`DecisionRecorded` pour que le [plan audit](./0001-audit-is-a-separate-evidence-plane.md) scelle la
justification DSA ; les événements d'enforcement existants centrés-offender restent intacts.

## Conséquences

- **Positives :** une réponse faisant autorité à « est-ce actionné ? » ; les voies coûteuses restent
  async et fail-open, donc la charge de moderation ne s'amplifie jamais sur les producteurs ; seule la
  porte `Screen` délibérément étroite est fail-closed ; la séparation propre d'avec classifieur /
  store / UI garde la politique découplée du ML.
- **Négatives / compromis accepté :** `Screen` est une dépendance synchrone sur le write path — elle
  doit être rapide et porter un timeout serré, et une panne de moderation bloque le *nouveau* contenu
  dans les catégories screenées (un compromis sécurité-sur-disponibilité accepté pour cette surface
  étroite).
- **Clôt :** la logique de moderation dispersée et non-autoritaire à travers les services de contenu.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Rendre `Screen` fail-open | Laisse passer du contenu nuisible précisément pendant une panne — inacceptable pour la T&S |
| Moderation comme classifieur | Couple la politique d'intégrité au cycle de vie du modèle ML ; confond « décider » et « détecter » |
| Intégrer l'état de moderation dans chaque service de contenu | Pas de SoR faisant autorité, pas d'enforcement cohérent, pas de piste de preuve unique |
