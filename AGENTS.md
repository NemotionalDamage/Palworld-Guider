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

## PHASES.md Content Rules

`PHASES.md` is a concise phase ledger, not an operational log or incident ticket.

Record only project-progress facts:

- current phase and overall project state
- phase table with status and purpose
- completed scope for the active or recently completed phase
- acceptance-gate results and durable verification summary
- blockers that prevent phase completion or require a product decision
- the next required action when a phase is not complete

Do not record:

- transient Git, network, authentication, build-machine, or shell failures after they are resolved
- push state, commit hashes, or routine commit/check noise
- raw command output, logs, secrets, timestamps unrelated to phase progress, or duplicate history
- environmental details that do not affect the current phase gate

Put detailed evidence, incident handling, and implementation notes in the appropriate `docs/phase-records/` or design document. If a transient incident creates a durable requirement, record that requirement in the relevant specification; do not preserve the incidental failure narrative in `PHASES.md`. Compress completed phase sections to their durable result.

## Git Backup And Version Control

The canonical repository is this directory. Its private GitHub remote is:

```text
git@github.com:NemotionalDamage/Palworld-Guider.git
```

- Keep the default working branch on `main` unless the user explicitly requests another branch.
- Commit after every completed phase, subphase, accepted fix, or documentation update that leaves the repository in a coherent state.
- After completing and committing a coherent change, ask the user whether to push. Push only after explicit user approval.
- Do not silently accumulate unpushed work across sessions; report the local commits that remain unpushed.
- Before committing code, run the relevant checks for the touched surface and record any intentionally skipped command.
- Use precise commit messages that name the phase or subsystem, for example `phase g0: establish guide project charter`.
- Never commit API keys, bearer tokens, cookies, local provider configuration, logs containing secrets, game assets, or wholesale copyrighted text.
- Keep generated artifacts, build directories, local logs, secrets, machine-specific paths, and unreviewed bulk reference dumps out of Git through `.gitignore`.
- If an approved push fails because of authentication or network access, report the failed attempt and the unpushed commit, then ask the user before retrying.
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

### Tier 0: Deterministic Knowledge Engine

- Load and validate a local, versioned, source-tracked knowledge base.
- Resolve item, Pal, recipe, technology, and alias identifiers deterministically.
- Answer exact lookup, recipe, material-tree, and shortage questions through Rust tools.
- Work offline without an LLM or live game state.
- Clearly report missing, stale, conflicted, or uncertain knowledge.

### Tier 1: Grounded Natural-Language Guide

- Understand natural-language questions through a bounded Rust agent loop.
- Combine structured lookup, lexical retrieval, and optional semantic retrieval.
- Let the LLM choose whitelisted tools and phrase answers, but never serve as the fact source.
- Keep every answer grounded in returned tool results and knowledge provenance.
- Enforce tool-call budgets, schemas, timeouts, and uncertainty reporting.

### Tier 2: State-Aware Advisor

- Combine knowledge with read-only or user-provided player state.
- Identify material shortages, craftable recipes, party work gaps, and likely next objectives.
- Distinguish observed state, user-entered state, assumptions, and unknowns.
- Fall back to general guidance when live state is unavailable.

### Tier 3: Progression Planner

- Maintain a goal graph covering materials, technology, crafting, Pal suitability, base setup, and progression stages.
- Recommend three to five prioritized next steps.
- Explain the reason, requirements, alternatives, and uncertainty for each recommendation.
- Respect player preferences such as combat, building, collection, automation, pace, and spoiler tolerance.

### Tier 4: Integrated Interfaces

- Receive natural-language questions through an in-game chat adapter.
- Return concise, grounded replies without leaving the game.
- Support follow-up questions and simple commands.
- Provide controlled, rate-limited proactive hints after explicit opt-in.

### Tier 5: Maintainable Knowledge System

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

- The project is a hybrid Rust RAG and Tool Use system, not a pure vector database or a framework-driven chatbot.
- Core knowledge schemas, validation, retrieval, ranking, deterministic calculators, planning, state interpretation, provider calls, agent orchestration, safety checks, interfaces, and tests must be implemented in Rust.
- Python projects, LangChain, AutoGen, Dify, and FastGPT may be studied as design references, but they must not become the production brain, tool registry, data pipeline, or agent runtime.
- A future game adapter may only expose read-only observation and chat input/output.
- The adapter must not own game semantics, planning, or advisory logic.
- The LLM may interpret language and phrase answers, but must not invent game facts.
- The LLM must not generate SQL, arbitrary file paths, unrestricted JSON queries, shell commands, or direct database access.
- All model-visible capabilities are typed, versioned tools registered in Rust.
- Tool calls require argument validation, result envelopes, timeouts, cancellation, and a maximum call budget.
- Exact facts and calculations are resolved before answer generation; the model must not overwrite tool-provided values.
- Every answer path should be able to identify:
  - the user question
  - the tools called
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

## Data And Retrieval Architecture

1. Reviewed source data is stored as versioned, diffable JSONL under `data/reviewed/`.
2. SQLite may be used as a derived read-only cache; it is not the canonical review artifact.
3. Do not introduce MySQL, MongoDB, or another networked database during the first usable release.
4. Retrieval is hybrid and ordered:
   1. exact identifier and alias resolution
   2. structured entity and relationship lookup
   3. lexical full-text search, normally Tantivy
   4. optional semantic vector search
   5. deterministic graph traversal for recipes, unlocks, drops, and breeding chains
5. Vector search is optional and supplementary. It must never override structured facts.
6. Embedding configuration must record provider, model, dimension, locale, normalization, and knowledge version.
7. Guide text should be transformed into reviewed summaries or factual chunks rather than copied wholesale from copyrighted pages.
8. When chunking is useful, use document-aware paragraphs of roughly 400–800 tokens with 10–20% overlap, preserve heading context, and attach source and version metadata.
9. Retrieval returns provenance and confidence; it does not return unreviewed text as settled fact.
10. A query must expose conflicts, stale versions, and missing records instead of selecting a silent winner.

## Tool Use Architecture

### Deterministic Tool Rules

- Recipes, material totals, shortages, craftable counts, breeding results, breeding chains, and work-capacity calculations must be pure Rust functions.
- The LLM must not perform arithmetic for user-visible quantities or guess breeding inheritance.
- Recursive material expansion must detect cycles, depth limits, duplicate ingredients, multiple outputs, by-products, and alternative recipes.
- Calculations must retain intermediate dependency trees so answers can show how totals were derived.

### Initial Tool Families

- Entity lookup: `resolve_name`, `get_item`, `get_pal`, `get_recipe`, `get_technology`.
- Relation lookup: `get_pal_work_suitability`, `get_pal_drops`, `get_pal_habitat`, `get_recipe_tree`.
- Calculators: `calculate_materials`, `calculate_shortage`, `calculate_craftable_count`, `calculate_breeding_result`, `calculate_breeding_chain`.
- Retrieval: `search_structured_knowledge`, `search_guide_text`, `get_conflicting_records`.
- State analysis: `import_player_snapshot`, `analyze_inventory`, `analyze_party`, `suggest_next_goals`.
- Optional private infrastructure status: `query_private_server_status`.

### Tool Boundaries

- The model sees only tool names, descriptions, and JSON schemas published by the Rust registry.
- Tool arguments are validated before execution.
- Tool results use a standard envelope containing status, data, provenance, version, uncertainty, and errors.
- The agent loop has a fixed maximum number of tool calls per question.
- Provider failure, retrieval failure, timeout, and invalid data produce clear errors without fabrication.
- Private server status is read-only and requires explicit configured ownership; public-server automation is prohibited.

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
Goal: build a reviewed, versioned, offline knowledge foundation.

Tasks:
1. Create the Rust workspace and the `game-knowledge` crate.
2. Design typed schemas for sources, provenance, items, recipes, technologies, Pals, work suitability, drops, habitats, breeding rules, aliases, conflicts, and progression relationships.
3. Define validation rules for identifiers, locale names, versions, units, ranges, references, and provenance.
4. Store canonical reviewed records as versioned JSONL and derive a read-only SQLite cache when useful.
5. Register every user-provided source in the source log before its facts are used.
6. Normalize the first reviewed dataset without copying wholesale copyrighted text.
7. Implement conflict records and provenance propagation.
8. Add schema, reference-integrity, version, alias, and conflict tests.

Acceptance:
- Every persisted fact has source, version, date, review status, and confidence.
- Invalid or incomplete records fail validation.
- Conflicts remain visible rather than being silently resolved.
- Canonical data remains diffable and reviewable.
- No LLM, retrieval index, or runtime advisor is required yet.
```

### Phase G2: Deterministic Guide CLI

```text
Goal: prove lookup and calculation correctness without an LLM or live game state.

Tasks:
1. Implement deterministic identifier and alias resolution.
2. Implement exact lookup for item use, acquisition leads, recipes, unlock requirements, Pal stats, drops, habitats, and work suitability.
3. Implement recursive material expansion, shortage calculation, and craftable-count calculation.
4. Implement deterministic breeding-result and breeding-chain tools once their rules are reviewed.
5. Add a CLI with commands such as `lookup`, `recipe`, `materials`, `shortage`, and `breeding`.
6. Detect cycles, depth limits, duplicate ingredients, multiple outputs, by-products, and alternative recipes.
7. Return uncertainty for missing, stale, or conflicted records.
8. Add property and regression tests for every calculator.

Acceptance:
- Common exact questions are answered from structured data.
- Recipe trees and quantity calculations are deterministic and reproducible.
- Shortage and craftable-count results show usable dependency trees.
- Missing knowledge returns `unknown` rather than a plausible fabrication.
- The guide works completely offline without an LLM.
- Tests cover aliases, ambiguous names, missing records, stale versions, cycles, and invalid quantities.
```

### Phase G3: Hybrid Retrieval And Grounded LLM Guide

```text
Goal: answer natural-language questions through bounded Rust tool use.

Tasks:
1. Add the `knowledge-index` and `guide-agent` crates.
2. Build a Tantivy lexical index over reviewed structured summaries and guide chunks.
3. Keep exact identifier, structured lookup, and graph traversal ahead of optional vector search.
4. If semantic search is added, record embedding provider, model, dimension, locale, normalization, and knowledge version.
5. Define the typed tool registry, JSON schemas, result envelope, timeouts, cancellation, and maximum tool-call budget.
6. Add OpenAI-compatible and Ollama providers; other providers may remain typed stubs.
7. Implement a bounded agent loop that classifies intent, resolves names, selects tools, and synthesizes answers.
8. Require deterministic tools for all numerical recipes, shortages, and breeding calculations.
9. Attach knowledge provenance, version warnings, conflicts, and unknowns to every answer path.
10. Add mock-provider and retrieval regression tests.

Acceptance:
- Natural-language questions can use exact lookup, lexical retrieval, and deterministic tools.
- The LLM cannot bypass the Rust tool registry.
- Model-generated arithmetic and breeding guesses never become user-visible facts.
- Retrieval returns a small, relevant, provenance-bearing context set.
- Provider, timeout, malformed-tool, and budget failures are clear and non-fatal.
- Missing knowledge returns `unknown`; stale or conflicting knowledge is visible.
```

### Phase G4: State-Aware Advisor And Progression Planner

```text
Goal: combine knowledge, explicit player state, and preferences into prioritized advice.

Tasks:
1. Add the `state-snapshot` and `guide-planner` crates.
2. Define a versioned read-only player-state schema for inventory, party, unlocks, goals, and preferences.
3. Distinguish observed state, user-entered state, assumptions, and unknowns.
4. Implement inventory-gap, party-work-gap, craftable-now, and goal-readiness analysis.
5. Model progression relationships and player preferences.
6. Rank three to five next steps by relevance, effort, benefit, risk, and uncertainty.
7. Explain requirements, alternatives, expected benefit, and data confidence for each recommendation.
8. Add mock-state, invalid-state, adversarial-goal, stale-knowledge, and conflicting-preference tests.

Acceptance:
- Recommendations are deterministic for the same knowledge and state.
- Every step has a reason and requirement basis.
- Advice references supplied state and clearly identifies missing state.
- Missing state degrades cleanly to general guidance.
- Spoiler-sensitive and long-horizon advice is controlled by preferences.
- No game I/O or mutation is performed.
```

### Phase G5: Integrated Interfaces

```text
Goal: expose the advisor through useful interfaces, ending with read-only in-game chat for the recorded target.

Tasks:
1. Add the `guide-server` crate with an Axum Web API and a minimal browser UI.
2. Preserve session state, follow-up context, provider configuration, rate limits, and answer provenance.
3. Add optional interface adapters only after the Web path is stable, such as Tauri, Discord, or QQ.
4. Define the target Palworld version, platform, load mode, save backup, and adapter constraints before live integration.
5. Add read-only state snapshot retrieval only where safely verifiable in the target build.
6. Add natural-language in-game input and concise reply output through a thin adapter.
7. Add timeouts, payload limits, redaction, cancellation, and clear provider, adapter, and retrieval errors.
8. Validate the final read-only chat path in a private local session.

Acceptance:
- The Web interface can answer grounded questions and show provenance and uncertainty.
- Optional adapters reuse the same Rust core and tool registry.
- A player question receives a useful grounded reply in game for the recorded target.
- Read-only capabilities fail clearly when unavailable.
- No mutation path exists.
```

### Phase G6: Knowledge Maintenance And Hardening

```text
Goal: keep the guide trustworthy across game updates and long sessions.

Tasks:
1. Add game-version, knowledge-version, index-version, and embedding-version compatibility checks.
2. Add source review, conflict resolution, and knowledge-update workflows.
3. Add answer regression suites and evaluation metrics for lookup accuracy, calculation correctness, retrieval hit rate, hallucination rate, and version-warning behavior.
4. Add performance budgets, log rotation, secret redaction, crash recovery, and backup documentation.
5. Document installation, configuration, troubleshooting, data updates, and interface deployment.

Acceptance:
- Version drift is visible to the user.
- Knowledge changes are auditable.
- Regressions protect common facts, recipe calculations, shortages, breeding chains, and natural-language answers.
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
6. Use the provided tools for exact facts, recipes, quantities, shortages, and breeding results.

Hard rules:
- Never invent game facts.
- Never claim a state was observed unless it is present in the supplied state.
- Never calculate material totals, recipe trees, shortages, or breeding results without calling the corresponding deterministic tool.
- Never change a value returned by a tool.
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
6. Do not adopt Python agent frameworks, hosted low-code platforms, or external orchestration services as the production brain.
7. Do not add live-game integration before Phase G5.
8. Treat every Palworld update as a knowledge-compatibility boundary.
9. Keep secrets outside Git.
10. Before finishing a code phase, normally run:
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test`
11. Report unresolved data uncertainty and skipped verification explicitly.

## Target Layout

```text
adapter/
  read-only/

crates/
  guide-core/
  game-knowledge/
  knowledge-index/
  guide-tools/
  guide-agent/
  guide-planner/
  provider/
  state-snapshot/
  guide-server/

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
