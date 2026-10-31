# ADR-0009: Maintain living, evidence-led architecture decisions

- **Status:** Accepted
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [ADR index](README.md)

## Context

A five-year architecture cannot treat every early hypothesis as permanent, nor can decisions be silently rewritten so that their original motivation disappears. The project needs lightweight documentation that stays current without becoming a monument to past assumptions.

## Decision

Maintain small, **living** ADRs with explicit status, review date, alternatives, consequences, validation and re-evaluation triggers. Accepted decisions guide changes; Proposed decisions are design hypotheses. Experimental decisions may be time-boxed in code but are not stable public contracts. Make substantial amendments in the original ADR with an amendment entry; use a new superseding ADR when the central decision changes. Update the index and relevant architecture summary together.

Attach evidence and reproduce benchmark claims where possible. Do not require an ADR for trivial refactoring, formatting or implementation details consistent with an existing decision. Decision records should guide implementation, not substitute for tested code.

## Consequences

+ Maintains a navigable record of principles and why they changed.
+ Enables architectural evolution without accidental scope commitments.
- Requires regular review during major work rather than one-time documentation.

## Alternatives considered

- **Immutable ADR archive:** useful history but can leave stale guidance as the apparent source of truth.
- **Single evolving architecture document only:** obscures trade-offs and accepted versus pending choices.
- **ADR for every detail:** high process cost and low signal.

## Validation

Each meaningful architecture PR should point to an existing ADR or amend/propose one. Review the index during the first real-codec integration and again when GPU work begins; remove unsubstantiated claims and reconcile contradictions.

## Review when

ADRs become repetitive or stale, or maintainers cannot determine the current policy from the index.

## Amendment log

- 2026-10-31 — Accepted living ADR process.
