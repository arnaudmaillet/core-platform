---
i18n:
  source: ./0000-template.md
  source_sha256: 2758b239613114431c6f9f634d80098756f1d58f33ecc15ed4db08e9094b20fd
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0000-template.md`](./0000-template.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants d'ADR et statuts restent en anglais.

# ADR-00NN : <titre court de décision>

<!--
 Copier vers docs/adr/00NN-<kebab-title>.md (prochain numéro libre, paddé à 4 chiffres).
 Les ADR sont IMMUABLES une fois Accepted : pour en inverser un, écrire un NOUVEL ADR qui le remplace
 et basculer le statut de celui-ci en « Superseded by ADR-00MM ». Ne jamais réécrire la décision en
 place — la valeur d'un ADR est la piste préservée du POURQUOI.
 Format : MADR allégé. Le garder à un écran. Le relier depuis le §9 du DOMAIN.md concerné.
-->

- **Statut :** Proposed | Accepted | Superseded by ADR-00MM | Deprecated
- **Date :** <AAAA-MM-JJ>
- **Contexte(s) affecté(s) :** <bounded contexts, ex. audit, moderation>
- **Décideurs :** <noms / guilde>

## Contexte et problème

<Les forces en tension : la pression de domaine, la forme de charge, la contrainte, la contradiction à
résoudre. 3–6 phrases. Énoncer le problème si bien que la décision se lise comme inévitable.>

## Décision

<Le choix, énoncé en règle au présent : « Nous … ». Un paragraphe. Assez spécifique pour qu'un
relecteur puisse dire si une future PR le viole.>

## Conséquences

- **Positives :** <ce que cela nous apporte>
- **Négatives / compromis accepté :** <ce qu'on abandonne sciemment>
- **Clôt :** <la contradiction ou le risque spécifique que ceci supprime>

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| `<l'alternative évidente>` | `<la raison disqualifiante>` |
| `<…>` | `<…>` |
