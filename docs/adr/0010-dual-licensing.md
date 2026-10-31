# ADR-0010: Dual-license Ferrixel under MIT OR Apache-2.0

- **Status:** Accepted
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [ADR index](README.md), [Project thesis](0001-project-thesis.md)

## Context

Ferrixel is intended as a long-lived Rust image-processing engine that can be used in open-source projects and proprietary software. The Rust ecosystem commonly offers permissive licensing under either MIT or Apache License 2.0. The project should be straightforward to reuse while preserving clear licensing terms and the explicit patent grant offered by Apache 2.0.

Prior to this decision the repository did not grant a redistribution license. Ferrixel has no published crate or stable API yet.

## Decision

License the original Ferrixel source code under **MIT OR Apache-2.0**, at the recipient's choice.

- Keep the complete applicable license texts in `LICENSE-MIT` and `LICENSE-APACHE`.
- Declare the SPDX expression `MIT OR Apache-2.0` in `Cargo.toml`, and state the dual-license choice in `README.md`.
- Contributions intentionally submitted to the project are made under the same dual-license terms unless explicitly stated otherwise. Contributors must have the rights necessary to make such submissions.
- Evaluate the licenses, notices, patent conditions, and redistribution requirements of third-party dependencies, codec libraries, fixtures, and copied code independently. The Ferrixel license does **not** automatically relicense those components.
- Treat publishing to crates.io and establishing a stable API as separate decisions.

The `OR` operator is intentional: recipients may elect either license; it does not require satisfying both simultaneously.

## Consequences

+ Broad reuse, including in proprietary and open-source products.
+ A well-recognized licensing convention for Rust libraries.
+ Apache License 2.0 provides an explicit patent grant with its own terms and obligations.
- Distributions must comply with the terms of the license chosen, including applicable notices and attribution.
- Dependency selection requires continued license diligence; permissive licensing cannot be assumed from a Rust implementation alone.

## Alternatives considered

- **Apache-2.0 only:** provides explicit patent terms, but removes the common MIT alternative for users who prefer it.
- **MIT only:** extremely concise and familiar, but lacks Apache-2.0's express patent grant.
- **Copyleft licensing:** legitimate for other project goals, but would add restrictions counter to Ferrixel's goal of broad adoption.
- **Unlicensed until release:** prevents legal reuse of an openly developed codebase during its design phase.

## Validation

Verify that the repository contains both complete license texts, that the Cargo metadata uses exactly `MIT OR Apache-2.0`, and that the README and contributor guidance agree. Check the package contents before the first crates.io release. Review third-party dependency licenses before adoption.

## Review when

Project ownership, contribution requirements, distribution strategy, or dependencies introduce a material new licensing constraint. Any license change must be recorded with its legal and contributor implications.

## Amendment log

- 2026-10-31 — Accepted permissive dual licensing under MIT OR Apache-2.0.
