# Phase G3 Record

## Scope Delivered

- Added the `knowledge-index` crate: an in-RAM Tantivy 0.24 lexical index over deterministic structured summaries for items, Pals, technologies, recipes, and habitats, with aliases attached to their targets, exact-title boosting, stored provenance, version metadata, small result sets, and explicit unknown/stale envelopes.
- Added the `guide-tools` crate: twelve typed tools (`resolve_name`, `get_item`, `get_pal`, `get_recipe`, `get_technology`, `search_structured_knowledge`, `calculate_materials`, `calculate_shortage`, `calculate_craftable_count`, `calculate_breeding_result`, `calculate_breeding_chain`, `get_conflicting_records`) with JSON schemas, argument validation, standard status/data/provenance/version/uncertainty/error envelopes, call budgets, and deadlines.
- Added the `provider` crate: a transport-neutral `ChatProvider` contract, OpenAI-compatible and Ollama blocking adapters with timeouts and a clear error taxonomy, pure request builders and response parsers, and a scripted mock provider. Secrets stay environment-only and out of errors.
- Added the `guide-agent` crate: a bounded agent loop using the Runtime Guide Prompt, publishing only registry tools, feeding envelopes back to the model, accumulating provenance/version/uncertainty, enforcing the tool budget, deadline, cancellation, reply-size limit, and exposing a `guide-agent ask` CLI with environment-only provider configuration.
- Exposed the existing deterministic resolver and added habitat accessors without changing G2 calculator semantics.
- Kept exact identifier and structured lookup ahead of lexical retrieval; semantic vector search remains deferred and no embedding metadata was added.
- Preserved the Stage 1 boundary: no game process, save file, server API, adapter, dynamic player state, or mutation path was added.

## Grounding And Safety Behavior

- The model sees only registry-published tool names, descriptions, and schemas; unknown tools and invalid arguments return error envelopes instead of executing.
- All numerical recipes, shortages, craftable counts, and breeding results come from the existing deterministic Rust functions; tool-call records retain the envelope data used for the answer.
- Tool results are appended verbatim as JSON envelopes so the model phrases answers from returned facts rather than replacing them.
- Missing knowledge returns `unknown`; stale versions, conflicts, and uncertainty propagate into the final answer envelope.
- Provider failures, malformed tool calls, budget exhaustion, timeouts, and cancellation return clear non-fatal envelopes.
- Answers with no tool evidence carry an explicit `no deterministic tool evidence` uncertainty flag.

## Verification

Red test checkpoints:

- `knowledge-index` tests initially failed because `KnowledgeIndex` and `IndexStatus` did not exist.
- Two index test fixtures initially used the wrong reviewed-data path and an evidence-less synthetic source; both were corrected before behavioral verification.
- A lexical-recall regression initially missed `ITEM_WOODEN_CLUB` for the query `wood`; item summaries now include output and byproduct recipe relationships, which is also better guide context.
- `guide-tools` tests initially failed because the registry types did not exist.
- A missing-`parent_b` regression initially produced an empty error string; argument combination now returns the first real validation error.
- `provider` tests initially failed because the chat contract and adapters did not exist.
- `guide-agent` tests initially failed because the agent types did not exist.
- An unknown-tool regression initially hid the tool error inside the call record without surfacing it in `AgentAnswer.errors`; final answers now aggregate distinct tool errors.

Final gates:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed with 84 tests across the workspace.
- `git diff --check` passed.

No checks were intentionally skipped.

## Durable Uncertainty

- The canonical dataset remains a deliberately small G1 seed set, so retrieval coverage and natural-language answer breadth are intentionally limited until further reviewed intake.
- Real OpenAI-compatible and Ollama endpoints were not exercised; provider correctness is covered by offline request builders, response parsers, and the scripted mock. Live provider validation remains outstanding.
- Retrieval is lexical only. Chinese matching relies on exact alias tokenization because no semantic or language-specific vector index was added.
- The registry, budget, and envelope guarantee the capability boundary, while answer phrasing still depends on the configured model honoring the tool protocol; weaker models may produce less useful grounded prose without being able to bypass the registry.
- No running game, save file, server API, or dynamic player state was used, matching the Stage 1 boundary.
