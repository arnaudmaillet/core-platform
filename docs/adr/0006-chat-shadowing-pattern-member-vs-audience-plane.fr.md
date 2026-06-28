---
i18n:
  source: ./0006-chat-shadowing-pattern-member-vs-audience-plane.md
  source_sha256: b45f9ec9d6e22b3a966b5a6567da8f7130dcd4a82ae87b3938e3230006e67ebc
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0006-chat-shadowing-pattern-member-vs-audience-plane.md`](./0006-chat-shadowing-pattern-member-vs-audience-plane.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0006 : Chat utilise le Shadowing Pattern — plans de visibilité Membre vs Audience séparés

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** chat
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

Une conversation a deux audiences distinctes avec des droits différents : les **membres** qui lisent
et écrivent, et une **audience** plus large qui peut voir une conversation *publiée* mais pas y
participer. Modéliser les deux avec un seul flag de visibilité fuit l'état membre-seul vers l'audience
(ou cache le contenu publié de celle-ci), et une conversation publiée puis dépubliée doit voir sa vue
audience démantelée proprement sans perturber le journal des membres.

## Décision

Chat modélise les deux comme **des plans séparés — le plan Membre et le plan Audience (le Shadowing
Pattern)**. L'appartenance est autorisée à la frontière gRPC pour le plan membre ; la publication
ouvre le plan audience ; la dépublication déclenche un `VisibilityWorker` qui consomme
`chat.conversation.unpublished` et démantèle l'état du plan audience. Le journal de messages est
stocké dans une partition ScyllaDB bucketée `(conversation_id, bucket)`, découplée de la visibilité.

## Conséquences

- **Positives :** l'état membre-seul ne fuit jamais vers l'audience ; publier/dépublier est une simple
  bascule de plan, pas une réécriture par-message ; le journal à fort volume scale par bucket.
- **Négatives / compromis accepté :** deux plans à garder cohérents, et un worker de démantèlement
  async (avec sa propre DLQ) à opérer.
- **Clôt :** la fuite de confusion de visibilité et le problème de démantèlement à la dépublication.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Un seul flag de visibilité par conversation | Fuit l'état membre vers l'audience ou cache le contenu publié |
| Vérifications ACL par-message à la lecture | Coûteux sur un journal à fort volume ; ne modélise pas le plan audience |
