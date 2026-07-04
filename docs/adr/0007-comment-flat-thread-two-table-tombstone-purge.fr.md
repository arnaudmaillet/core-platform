---
i18n:
  source: ./0007-comment-flat-thread-two-table-tombstone-purge.md
  source_sha256: 2f4d26d34af653f25b9f33694dd83af6046c971533bc4bd0fb717cbe525b57eb
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0007-comment-flat-thread-two-table-tombstone-purge.md`](./0007-comment-flat-thread-two-table-tombstone-purge.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0007 : Comment utilise un fil plat nil-UUID, un layout Scylla deux tables et une suppression tombstone-vs-purge

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** comment
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Les commentaires sont un flux à forte écriture qui doit être lu à la fois par id et en ordre
temporel, et la suppression a deux sens réellement différents : un utilisateur retirant son propre
commentaire (laisser un marqueur « supprimé » visible) versus un retrait dur modération/RGPD (la ligne
doit disparaître). Une seule sémantique de suppression ne peut servir les deux, et un seul motif
d'accès ne peut servir à bas coût à la fois le lookup-par-id et les lectures ordonnées dans le temps.

## Décision

Comment modélise un **fil plat utilisant un sentinelle nil-UUID** pour les commentaires sans racine,
les stocke dans un **layout ScyllaDB à deux tables** (une table LCS pour les lookups par id + une
table TWCS pour le flux ordonné dans le temps), et fait de la suppression une **`DeletionStrategy`
explicite — tombstone (marqueur visible) vs purge (retrait dur)**.

## Conséquences

- **Positives :** les deux motifs de lecture sont bon marché et avec la compaction appropriée (LCS vs
  TWCS) ; la sémantique de suppression correspond aux cas d'usage réels ; le sentinelle nil-UUID garde
  le modèle plat et simple.
- **Négatives / compromis accepté :** des écritures à deux tables à garder cohérentes ; pas de
  threading imbriqué (une simplification délibérée).
- **Clôt :** l'ambiguïté de sémantique de suppression et le mismatch de motifs de lecture.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Table unique | Ne peut servir lookup-par-id et lectures ordonnées dans le temps avec la compaction appropriée |
| Un seul flag « deleted » | Confond la suppression-utilisateur (tombstone) avec le purge modération/RGPD |
| Arbre de fil imbriqué maintenant | Complexité prématurée pour le produit actuel |
