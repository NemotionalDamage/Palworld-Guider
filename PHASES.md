# Palworld Guider Phase Progress

Last updated: 2026-08-30, Asia/Shanghai.

## Current Status

Phase G0 is complete. This repository is established as an independent guide-first project. The governing documents, source policy, and reference-intake baseline are in place. No Rust implementation, game adapter, or detailed game dataset exists yet.

The G1–G6 roadmap has been refined around a hybrid Rust RAG and deterministic Tool Use architecture: reviewed structured data first, exact Rust calculators second, grounded natural-language retrieval third, state-aware planning fourth, and integrated interfaces last.

The initial documentation commit is preserved locally. Pushing to GitHub is currently blocked because this machine has no usable SSH key for git@github.com.

The older `Pal` project remains separate. Its control-oriented roadmap does not govern this repository.

## Phase Summary

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Establish the independent charter, roadmap, and reference-data rules |
| G1 | Not started | Define knowledge schemas and ingest reviewed sources |
| G2 | Not started | Build the deterministic offline guide CLI |
| G3 | Not started | Add hybrid retrieval and grounded LLM answers |
| G4 | Not started | Add state-aware advice and progression planning |
| G5 | Not started | Build Web and read-only in-game interfaces |
| G6 | Not started | Harden versioning, knowledge maintenance, and operations |

## Phase G0 Progress

Completed:

- Created an independent project root.
- Defined the guide-first product mission.
- Defined the read-only capability ladder and non-goals.
- Defined the Rust core architecture boundary.
- Created the knowledge-source policy.
- Created the reference-data source log and intake template.
- Added the project-owner-supplied candidate reference-source catalog to `AGENTS.md`.
- Refined the architecture and G1–G6 roadmap around hybrid Rust RAG and deterministic Tool Use.
- Initialized the Git repository on `main`.
- Connected the provided private GitHub remote.

Remaining:

None.

### Verification

Documentation-only phase. Verification consists of repository structure, content review, source-catalog review, remote configuration, and Git status review; no Rust checks apply yet.

### Blockers

Remote backup is blocked: GitHub SSH authentication returns Permission denied (publickey). The local main branch is intact. Push can resume after a usable SSH key is configured for the repository owner or after authenticated HTTPS credentials are supplied.

## Phase Boundaries

- No game API is called.
- No adapter is implemented.
- No game fact is asserted beyond the current documentation boundary.
- No world mutation or player automation is planned.
