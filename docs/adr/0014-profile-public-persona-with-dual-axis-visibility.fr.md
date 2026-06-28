---
i18n:
  source: ./0014-profile-public-persona-with-dual-axis-visibility.md
  source_sha256: cd78ea432c8d970d70c1285d1e1dba6389e729cee0da78534f863d4e0ecea1d0
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`0014-profile-public-persona-with-dual-axis-visibility.md`](./0014-profile-public-persona-with-dual-axis-visibility.md) fait foi.
> En cas de divergence, l'anglais prime. Les identifiants, codes, noms de types et statuts restent en anglais.

# ADR-0014 : Profile est le persona public au-dessus du SoR account avec visibilité bi-axiale

- **Statut :** Accepted
- **Date :** 2026-06-26
- **Contexte(s) affecté(s) :** profile ; account ; aval post, search, geo-discovery, timeline
- **Décideurs :** arnaudmaillet (architecture)

## Contexte et problème

La face publique d'un utilisateur (handle, display name, bio, avatar, tier) est une préoccupation
différente du compte privé de référence (identifiants, PII, KYC). Les regrouper met la PII dans le
store de persona à lecture chaude. Deux parties distinctes peuvent aussi légitimement masquer un
profil — le **propriétaire** (vie privée) et la **modération** (enforcement) — et un seul flag de
visibilité ne peut représenter les deux sans que l'un prime silencieusement sur l'autre. Les handles
doivent être globalement uniques sous des revendications concurrentes.

## Décision

`profile` est le **système de référence du persona public**, superposé au SoR `account` (provisionné
en consommant `account.v1.events`) et ne détenant aucune PII. La visibilité est **bi-axiale** : un
profil n'est visible que si le **propriétaire ET la modération** l'autorisent tous deux. Les handles
sont **globalement uniques avec des revendications sans conflit** sous concurrence. Profile **possède
et émet** `profile.v1.events` (y compris `handle_changed`, `profile_verified` et `tier_changed`) pour
le côté lecture ; le **tier d'auteur est calculé par `social-graph`** mais possédé et publié ici.

## Conséquences

- **Positives :** la PII reste dans `account` ; les lectures de persona sont chaudes et sans PII ; les
  deux autorités-de-masquage sont représentées indépendamment ; les read-models aval réagissent via
  événements.
- **Négatives / compromis accepté :** le persona doit rester cohérent-à-terme avec account ; la voie
  de revendication de handle nécessite une gestion atomique de l'unicité.
- **Clôt :** la PII-dans-le-store-de-persona et le problème de prime du flag-de-visibilité-unique.

## Alternatives rejetées

| Option | Pourquoi rejetée |
|---|---|
| Un seul service pour account + profile | Met la PII dans le store de persona à lecture chaude |
| Flag de visibilité unique | Les masquages propriétaire et modération priment silencieusement l'un sur l'autre |
| Calculer/posséder le tier dans profile | Le tier dérive du graphe de followers (`social-graph`) |
