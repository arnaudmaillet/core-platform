# Ubiquitous Language (cross-context)

> 🟡 **Scaffold.** This glossary holds **only** terms used by more than one bounded context.
> Terms that live inside a single context belong in that service's `DOMAIN.md §2`, not here.

## Why a cross-context glossary

The same word often means different things in different contexts ("post," "user," "delivery"),
and a few words must mean *exactly the same thing everywhere* (identifiers, tier labels). This
file pins down the second category and flags the first, so an event or RPC consumed across a
boundary is not silently misread.

## Rules

- **Code symbol is mandatory.** A term with no `crate::Type` / proto / topic is aspirational,
  not ubiquitous — leave it out until it has one.
- **Contracts stay in English.** Per the [translation standard](../i18n/TRANSLATION.md),
  identifiers, error codes, topics, env vars, and type names are language-invariant.
- **Note divergence.** When a word means different things in different contexts, give it a row
  per context and say so explicitly.

## Shared terms (one meaning everywhere)

| Term | Meaning | Code symbol / contract | Owning context |
|---|---|---|---|
| `<Term>` | `<the single platform-wide meaning>` | `<symbol / topic / proto>` | `<ctx>` |

## Overloaded terms (different meaning per context)

| Term | Context | Meaning here | Code symbol |
|---|---|---|---|
| `<Term>` | `<ctx-A>` | `<meaning in A>` | `<symbol>` |
| `<Term>` | `<ctx-B>` | `<meaning in B>` | `<symbol>` |
