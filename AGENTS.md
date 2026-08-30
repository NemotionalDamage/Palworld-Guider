# Palworld Guider Agent Instructions

This repository implements an in-game guide and advisor for Palworld. The agent helps players understand items, materials, Pals, mechanics, and progression, then gives personalized next-step advice. It is not an autonomous player.

## Standing Operating Contract

- Treat this file as the durable project specification. Do not replace it without an explicit user request.
- Maintain `PHASES.md` in real time whenever a phase starts, a gate passes or fails, a blocker changes, or a phase completes.
- Read this file before changing code, documentation, configuration, tests, or data.
- Execute only the phase explicitly requested. If no phase is specified, ask which phase to run.
- A phase is complete only when all acceptance criteria are satisfied. If a criterion cannot be satisfied, stop and document the blocker rather than starting a later phase.
- Prefer offline knowledge-engine and mock-state tests during normal development. Use a real game only for final read-only validation.
- Report unresolved game-data uncertainty, live-game limitations, and version-compatibility risks explicitly.

## Git Backup And Version Control

The canonical repository is this directory. Its private GitHub remote is:

```text
git@github.com:NemotionalDamage/Palworld-Guider.git
```

- Keep the default working branch on `main` unless the user explicitly requests another branch.
- Commit after every completed phase, subphase, accepted fix, or documentation update that leaves the repository in a coherent state.
- Push to the private remote after every commit intended to be preserved. Do not accumulate unpushed work across sessions.
- Before committing code, run the relevant checks for the touched surface and record any intentionally skipped command.
- Use precise commit messages that name the phase or subsystem, for example `phase g0: establish guide project charter`.
- Never commit API keys, bearer tokens, cookies, local provider configuration, logs containing secrets, game assets, or wholesale copyrighted text.
- Keep generated artifacts, build directories, local logs, secrets, machine-specific paths, and unreviewed bulk reference dumps out of Git through `.gitignore`.
- If authentication or network access prevents pushing, commit locally when safe, clearly report the unpushed commit, and retry before ending the session.
- Do not rewrite published history or force-push without explicit user approval.

## Product Mission

Build a trustworthy, accessible Palworld guide that lives close to the player's current game context:

1. Explain what items and materials are for.
2. Explain how to obtain specific items and materials.
3. Interpret the player's current inventory, party, and available read-only state.
4. Identify gaps between the player's current state and a stated goal.
5. Recommend a small set of useful next steps with clear reasons.
6. Keep answers concise, factual, version-aware, and suitable for in-game chat.

The first usable milestone is an offline guide backed by a versioned knowledge base. Live game integration is read-only and comes only after the offline guide is useful.

## Product Principles

- Reduce context switching: the player should not need to leave the game to understand basic progression choices.
- Personalize advice: combine game knowledge with the player's actual state when that state is available.
- Prefer explanation over automation: the agent recommends actions; it does not replace the player.
- Ground every fact: concrete recipes, drop sources, stats, mechanics, and unlock requirements must come from reviewed structured data.
- Never fake certainty: missing or stale data must produce an explicit uncertainty warning or `unknown`.
- Keep the first release read-only: no movement, combat, gathering, construction, inventory mutation, or world mutation.
- Keep the interface low-friction: short answers by default, with optional deeper explanation.

## Expected Capability Ladder

### Tier 0: Offline Knowledge Guide

- Answer common questions about item use, material sources, recipes, Pal work suitability, and basic mechanics.
- Use a local, versioned, source-tracked knowledge base.
- Work without live game state.
- Clearly report missing or uncertain knowledge.

### Tier 1: State-Aware Advisor

- Combine knowledge with read-only player state.
- Identify material shortages, craftable recipes, party work gaps, and likely next objectives.
- Distinguish observed state from user-provided state.
- Fall back to general advice when live state is unavailable.

### Tier 2: Progression Planner

- Maintain a goal graph covering materials, technology, crafting, Pal suitability, base setup, and progression stages.
- Recommend three to five prioritized next steps.
- Explain the reason, requirements, alternatives, and uncertainty for each recommendation.
- Respect player preferences such as combat, building, collection, automation, pace, and spoiler tolerance.

### Tier 3: Integrated Chat Guide

- Receive natural-language questions through an in-game chat adapter.
- Return concise, grounded replies without leaving the game.
- Support follow-up questions and simple commands.
- Provide controlled, rate-limited proactive hints after explicit opt-in.

### Tier 4: Maintainable Knowledge System

- Detect game-version and knowledge-version mismatches.
- Record source, extraction date, applicable game version, confidence, and review status.
- Provide regression tests for common answers.
- Support auditable knowledge updates after Palworld patches.

## Non-Goals

- Autonomous or unsupervised play.
- Automatic movement, combat, gathering, construction, or item transfer.
- Public-server automation or anti-cheat bypass.
- Replacing the game's existing tutorials for basic controls.
- Using an LLM as the authoritative source of game facts.
- Copying game assets or large copyrighted text into the repository.
- General conversational personality features before guide quality is reliable.

## Required Architecture

- Core knowledge, retrieval, planning, ranking, provider calls, state interpretation, safety checks, and tests must be implemented in Rust.
- A future game adapter may only expose read-only observation and chat input/output.
- The adapter must not own game semantics, planning, or advisory logic.
- The LLM may interpret language and phrase answers, but must not invent game facts.
- Every answer path should be able to identify:
  - the user question
  - the knowledge records used
  - the live or user-provided state used
  - unresolved uncertainty
- All future mutating capabilities require a new explicit specification and are outside the current roadmap.

## Knowledge Rules

1. Structured knowledge is the source of truth for game facts.
2. Every persisted fact must carry provenance.
3. Preferred source order:
   1. Local data extracted from the exact target game build
   2. Official patch notes or official documentation
   3. User-reviewed secondary sources
   4. Community references
4. LLM output is not evidence. It may generate hypotheses only during interactive explanation, never persisted facts.
5. Knowledge records must include:
   - applicable Palworld version
   - source identifier
   - retrieval or extraction date
   - review status
   - confidence
   - change risk where relevant
6. Version-sensitive data must warn when its knowledge version does not match the current game version.
7. Conflicting sources require an explicit conflict record; do not silently choose one.

## Research And Data Boundary

- Palworld is frequently updated. Community guides can be stale, incomplete, or version-specific.
- Do not treat search-result claims, model memory, or unreviewed community pages as verified facts.
- Do not store raw copyrighted page dumps, game assets, or excessive quoted text.
- Record useful source URLs and metadata in `docs/reference-data/source-log.md`.
- Normalize only the facts needed by the guide into reviewed structured data.
- When the user provides reference material, register it in the source log before using it as evidence.

### Candidate Reference Sources

The following sources were supplied by the project owner on 2026-08-30. They are candidate leads only. Listing them does not verify accuracy, license, coverage, availability, or current URL. Before a source supports a persisted fact, register and review it through `docs/reference-data/source-log.md`.

#### Comprehensive Databases

| Source | Entry point | Claimed strength | Intended guide use |
|---|---|---|---|
| Palworld Database Wiki | `https://paldb.cc/` | Detailed item, weapon, material, unlock, and Pal data with clear categories. | Community lead for item use, recipes, materials, and unlock metadata. |
| Palworld.gg | `https://palworld.gg/` | Pal, item, recipe, building, and breeding data plus calculators. | Cross-check for recipes, materials, Pals, buildings, and calculators. |
| OP.GG Palworld database | `https://op.gg/` | Item recipes, values, dropping Pals, prices, and a Pal codex. | Cross-check for item stats, recipes, drops, prices, and Pal data; confirm the exact Palworld section before use. |

#### Chinese Wikis And Guides

| Source | Entry point | Claimed strength | Intended guide use |
|---|---|---|---|
| BWIKI 幻兽帕鲁 Wiki | `https://wiki.biligame.com/palworld` | Chinese item categories, Pal codex, and base-building guides. | Chinese terminology, item descriptions, Pal names, and localized explanations. |
| Palworld Wiki (GameVault) | `https://palworld.gamevault.in` | Chinese survival guide covering basics, objectives, HUD, and mechanics. | Chinese onboarding and mechanics explanations. |
| Gamersky Palworld guides | `https://www.gamersky.com/` | Chinese guides and a 1.0 Pal codex with attributes and drops. | Chinese terminology, Pal attributes, drops, and progression guides; locate the specific article before citing. |

#### Gameplay Guide Sites

| Source | Entry point | Claimed strength | Intended guide use |
|---|---|---|---|
| Game8 | `https://game8.co/` | Craftable-item lists, unlock requirements, materials, mechanics, and building guides. | Cross-check for recipes, unlock requirements, mechanics, and progression advice; confirm the Palworld hub before use. |
| IGN Palworld guides | `https://www.ign.com/` | Broad early-game through late-game progression guides. | Progression structure, goal framing, and explanatory language; locate the specific Palworld guide before citing. |
| GameWith | `https://gamewith.ai/` | Recipe and material calculation pages with dependency trees and totals. | Material dependency trees, shortage calculations, and acquisition advice; confirm the Palworld section before use. |

#### Developer And Technical Documentation

| Source | Entry point | Claimed strength | Intended guide use |
|---|---|---|---|
| Palworld Server Guide | `https://docs.palworldgame.com/` | Official server guide with technology IDs and experimental REST API documentation. | Official technology identifiers, server mechanics, and future technical integration research. |
| PalSchema Hub | `https://www.nexusmods.com/` | Community schema database for editable Pal, item, recipe, and building fields. | Field-name and schema leads for local-data extraction; confirm the exact hub page and license before use. |

#### Open Data And API Projects

| Source | Entry point | Claimed strength | Intended guide use |
|---|---|---|---|
| Paldex | `https://github.com/blaynem/paldex` | Open-source translated companion app with generated game-file data and an API. | Open-data schema and extraction approach; review license, update process, and data version. |
| palworld-paldex-api | `https://github.com/nonopolarity/palworld-paldex-api` | Open-source API for Pal codex data. | Pal-data schema and API design reference; review license and freshness. |
| palworld-api | `https://pypi.org/project/palworld-api/` | Python REST API client package. | Client behavior and endpoint reference only; the guide core remains Rust. |

## Phase Workflow

Each phase has a prompt below. Use only the prompt for the requested phase.

### Phase G0: Project Charter And Reference Intake Baseline

```text
Goal: establish the independent guide project and its evidence boundary.

Tasks:
1. Create the independent repository and governing documents.
2. Define the product mission, capability ladder, non-goals, and architecture.
3. Establish the source-log and reference-intake format.
4. Record that no detailed game data has been reviewed yet.
5. Keep this phase documentation-only.

Acceptance:
- The new project is independent from the older control-oriented project.
- The guide-first, read-only product goal is explicit.
- Future game data has a mandatory provenance format.
- No implementation or game claims are added.
```

### Phase G1: Knowledge Schema And Source Intake

```text
Goal: define the structured representation for guide knowledge and ingest the first reviewed source set.

Tasks:
1. Design typed schemas for items, recipes, materials, technologies, Pals, work suitability, sources, and progression relationships.
2. Define validation rules for identifiers, versions, units, ranges, and provenance.
3. Build a reproducible offline data pipeline.
4. Register all user-provided sources.
5. Normalize the first reviewed dataset.
6. Add tests for schema validation and conflicting-source handling.

Acceptance:
- Every persisted fact has source, version, date, review status, and confidence.
- Invalid or incomplete records fail validation.
- Conflicts remain visible rather than being silently resolved.
- No runtime advisor is required yet.
```

### Phase G2: Offline Guide MVP

```text
Goal: answer common guide questions without live game state.

Tasks:
1. Implement deterministic knowledge retrieval and resolution.
2. Support questions for item use, acquisition, recipes, and Pal work suitability.
3. Add intent parsing suitable for short chat queries.
4. Return concise answers with uncertainty when data is missing or stale.
5. Add regression tests for representative questions.

Acceptance:
- Common factual questions are answered from structured data.
- Missing knowledge returns `unknown` rather than a plausible fabrication.
- The guide works offline.
- Tests cover aliases, ambiguous names, missing records, and stale versions.
```

### Phase G3: State-Aware Advisor

```text
Goal: combine knowledge with an explicit player-state snapshot.

Tasks:
1. Define a versioned read-only player-state schema.
2. Implement inventory, party, and goal-gap analysis.
3. Distinguish observed state, user-entered state, assumptions, and unknowns.
4. Answer what is craftable, what is missing, and whether an item is useful now.
5. Add mock-state and invalid-state tests.

Acceptance:
- Advice references the supplied state.
- Missing state degrades cleanly to general guidance.
- The advisor performs no game I/O and no mutation.
```

### Phase G4: Progression Planner

```text
Goal: recommend prioritized next steps from a goal graph and player preferences.

Tasks:
1. Model progression relationships and player preferences.
2. Rank three to five next steps by relevance, effort, benefit, and uncertainty.
3. Explain requirements and alternatives.
4. Add adversarial tests for bad goals, stale knowledge, and conflicting preferences.

Acceptance:
- Recommendations are deterministic for the same knowledge and state.
- Every step has a reason and requirement basis.
- Spoiler-sensitive and long-horizon advice is controlled by preferences.
```

### Phase G5: Integrated Chat Guide

```text
Goal: connect the advisor to a read-only in-game chat interface for the recorded local target.

Tasks:
1. Define the target Palworld version, platform, load mode, and adapter constraints.
2. Add read-only state snapshot retrieval where safely verifiable.
3. Add natural-language input and concise reply output.
4. Add timeouts, rate limits, redaction, and clear provider/adapter errors.
5. Validate in a private local session.

Acceptance:
- A player question receives a useful grounded reply in game.
- Read-only capabilities fail clearly when unavailable.
- No mutation path exists.
```

### Phase G6: Knowledge Maintenance And Hardening

```text
Goal: keep the guide trustworthy across game updates and long sessions.

Tasks:
1. Add game-version and knowledge-version compatibility checks.
2. Add source review and update workflows.
3. Add answer regression suites and evaluation metrics.
4. Add log rotation, secret redaction, crash recovery, and backup documentation.
5. Document installation, troubleshooting, and data-update procedures.

Acceptance:
- Version drift is visible to the user.
- Knowledge changes are auditable.
- Formatting, lint, tests, and answer regressions pass.
```

## Runtime Guide Prompt

Use this as the base system prompt for the future LLM interface:

```text
You are the Palworld Guider brain.

Your responsibilities:
1. Understand the player's question or goal.
2. Use only the supplied structured game knowledge and player state as facts.
3. Explain items, materials, Pals, mechanics, and progression choices.
4. Recommend a small number of useful next steps.
5. Report missing, conflicting, or version-stale information clearly.

Hard rules:
- Never invent game facts.
- Never claim a state was observed unless it is present in the supplied state.
- Never recommend cheats, exploits, public-server automation, or anti-cheat bypass.
- Keep answers concise and suitable for in-game chat.
- Prefer a short answer with an optional detailed follow-up.
- If knowledge is missing, say what is unknown and what would be needed.

Response style:
- Be direct and practical.
- Explain why a recommendation matters.
- Mention material gaps and alternatives when useful.
- Do not use promotional language or repeat the player's question.
```

## Working Rules

1. Read existing docs, code, tests, and data before editing.
2. Execute only the requested phase.
3. Prefer small, reviewable changes.
4. Add or update tests for schemas, retrieval, state analysis, planner, and answer behavior.
5. Do not promote community data to reviewed status without source registration and review.
6. Do not add live-game integration before Phase G5.
7. Treat every Palworld update as a knowledge-compatibility boundary.
8. Keep secrets outside Git.
9. Before finishing a code phase, normally run:
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test`
10. Report unresolved data uncertainty and skipped verification explicitly.

## Target Layout

```text
adapter/
  read-only/

crates/
  guide-core/
  game-knowledge/
  guide-planner/
  provider/
  state-snapshot/

docs/
  product-goal.md
  knowledge-source-policy.md
  environment.md
  reference-data/
  schemas/
  phase-records/

data/
  reviewed/
  conflicts/

tests/
  fixtures/
```
