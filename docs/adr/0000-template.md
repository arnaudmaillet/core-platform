# ADR-00NN: <short decision title>

<!--
 Copy to docs/adr/00NN-<kebab-title>.md (next free number, zero-padded to 4 digits).
 ADRs are IMMUTABLE once Accepted: to reverse one, write a NEW ADR that supersedes it and
 flip this one's status to "Superseded by ADR-00MM". Never rewrite the decision in place —
 the value of an ADR is the preserved trail of WHY.
 Format: MADR-lean. Keep it to one screen. Link it from the relevant DOMAIN.md §9.
-->

- **Status:** Proposed | Accepted | Superseded by ADR-00MM | Deprecated
- **Date:** <YYYY-MM-DD>
- **Context(s) affected:** <bounded contexts, e.g. audit, moderation>
- **Deciders:** <names / guild>

## Context and problem

<The forces in tension: the domain pressure, the load shape, the constraint, the contradiction
that has to be resolved. 3–6 sentences. State the problem so well that the decision reads as
inevitable.>

## Decision

<The choice, stated as a present-tense rule: "We …". One paragraph. Be specific enough that a
reviewer can tell whether a future PR violates it.>

## Consequences

- **Positive:** <what this buys us>
- **Negative / accepted trade-off:** <what we knowingly give up>
- **Closes:** <the specific contradiction or risk this removes>

## Alternatives rejected

| Option | Why rejected |
|---|---|
| `<the obvious alternative>` | `<the disqualifying reason>` |
| `<…>` | `<…>` |
