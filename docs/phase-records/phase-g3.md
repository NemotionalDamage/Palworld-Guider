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
- When calculator tools are invoked, numerical recipes, shortages, craftable counts, and breeding results come from the existing deterministic Rust functions; tool-call records retain the envelope data. Final prose must restate an exact result leaf in the question's numeric-word locale.
- Tool results are appended verbatim as JSON envelopes so the model phrases answers from returned facts rather than replacing them.
- A deterministic grounding gate compares complete quantity expressions and reviewed entity claims against claim-specific successful evidence. It blocks serialized version digits, subset number words, scaled, fractional, scientific-notation, and locale-mismatched expressions, unrelated lookups, generic breeding conclusions without a tool result, and breeding arguments from a different parent pair. Explicit unknown answers may contain only uncertainty language and the requested parent entities. After the sixth audit correction, a numeric claim is bound to the entity phrase immediately following the number: the gate takes the maximal run of adjacent words that form a prefix of a known entity name, so comma-separated clauses and trailing context can no longer leak a prior entity into the claim's evidence. After the seventh audit correction, explicit unknown breeding answers allow permitted parent entities only as context: each permitted entity must be preceded by a context marker (`for`/`of`/`between`/`and`/`requested pair`-style connectors) or stand at the answer start, and the remaining words come from a stricter uncertainty vocabulary that excludes offspring-assertion words.
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
- The completion-audit regressions initially failed exactly as audited: numeric tampering (`15` rewritten to `12`) and model-authored calculations without tool evidence both returned `ok`, a breeding guess beyond an unknown tool result remained visible, and a no-tool answer still carried `ok` status.

Final gates:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed with 84 tests across the workspace.
- `git diff --check` passed.

No checks were intentionally skipped.

## 2026-08-31 Completion Audit

Fresh verification again passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 84 tests, and `git diff --check`. The repository was clean.

The audit found one acceptance failure: `GuideAgent::finalize` returns model-authored prose directly. It only flags answers that used no tools; it does not detect or reject calculation/breeding claims without a corresponding calculator call, nor does it compare numeric or breeding values in the final prose with successful deterministic tool results. Therefore the prompt and tool evidence reduce fabrication risk but do not satisfy the guarantee that model-generated arithmetic and breeding guesses never become user-visible facts.

G3 is not complete until a deterministic final-answer grounding gate and red/green regressions cover both unsupported model-authored calculation claims and tool-result contradictions.

## 2026-08-31 Completion Audit Correction

The audited blocker was reproduced with four red regressions:

- `numeric_tampering_is_rejected_by_grounding_gate` failed because a model answer of `12 Wood` was returned as `ok` alongside the tool-verified value of `15`.
- `unsupported_calculation_claims_require_tool_evidence` failed because `20 Wood` became visible without any calculator tool result.
- `breeding_guesses_beyond_tool_evidence_are_rejected` failed because a model claim that Lamball breeding produces Wool stayed visible after the breeding tool returned `unknown`.
- `answers_without_tool_evidence_are_flagged` failed because a no-tool answer still carried `ok` status.

Correction:

- Added `ToolRegistry::known_entity_names` from reviewed item, Pal, technology, and alias names.
- Added the deterministic grounding gate in `GuideAgent::finalize`: every numeric token in the final text must occur in successful tool data, and every known entity name must occur in successful tool data or the original tool arguments, which permits restating question entities without permitting new facts.
- Gate violations suppress the model answer, return `error`, and list each unsupported claim while retaining the tool-call records and provenance.
- No-tool answers now return `unknown` status in addition to the explicit no-evidence uncertainty.
- Unknown breeding answers may restate only the parent entities supplied in tool arguments.

Fresh gates after the correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 88 tests, and `git diff --check` all passed.

## 2026-08-31 Second Completion Audit

The corrected repository again passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 88 tests, and `git diff --check`.

Three temporary regressions were then run against the current agent to test the acceptance boundary directly:

- A `get_item(Wood)` call followed by `You need 3 Wood to craft a club.` returned the text visibly. The digit `3` was authorized by the serialized `1.0.3` version metadata rather than a calculator result.
- A `get_item(Wood)` call followed by `You need twenty Wood to craft a club.` returned the text visibly because `digit_runs` recognizes only ASCII digits and does not parse number words.
- An unknown `calculate_breeding_result(Lamball, Lamball)` followed by successful `get_item(Wool)` and `Lamball plus Lamball produces Wool.` returned the text visibly because entity evidence from the unrelated item lookup was pooled with the failed breeding evidence.

The temporary test command failed all three regressions with visible model answers. The temporary tests were removed after capturing the failures; a future correction must add equivalent permanent tests before changing the gate.

Root cause: the gate authorizes tokens globally rather than validating claim type, relevant tool identity, and tool-result status. Version metadata must not be numeric evidence; number words and non-ASCII numerals require claim-aware normalization; and breeding conclusions must be authorized only by a successful deterministic breeding result.

## 2026-08-31 Second Completion Audit Correction

The three audited bypasses were converted into five permanent red regressions:

- `version_digits_do_not_authorize_quantities`
- `number_words_are_normalized_and_require_calculator_evidence`
- `non_ascii_numerals_are_normalized_and_grounded`
- `unrelated_lookup_cannot_wrap_unknown_breeding_result`
- `breeding_entity_evidence_ignores_unrelated_arguments`

Correction:

- Numeric evidence now follows claim type. If any calculator tool was requested, only successful calculator-family result data can authorize visible quantities; otherwise successful non-calculator tool data remains usable for directly returned identifiers and counts. Serialized version metadata is excluded from numeric evidence.
- Numeric claims are normalized from number words through ninety and fullwidth Unicode digits before comparison with deterministic evidence.
- If any breeding calculator was requested, entity evidence is restricted to successful breeding-tool data plus breeding-tool arguments. Unrelated lookup results and arguments cannot wrap an unknown breeding result.
- Calculation and breeding context violations independently require a successful calculator or breeding result, including the context-free breeding-entity case.
- Gate violations continue to suppress the model answer, return `error`, and retain tool-call records, provenance, and explicit unsupported-claim diagnostics.

Fresh gates after the second correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 93 tests, and `git diff --check` all passed.

## 2026-09-01 Third Completion Audit

The second-correction repository passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 93 tests, and `git diff --check`.

Four temporary regressions then tested the arithmetic and breeding acceptance boundary:

- After a successful `calculate_materials(Wooden Club, 3)`, `You need fifteen thousand Wood.` remained visible because `fifteen` normalized to the authorized value `15` while `thousand` was ignored.
- After the same calculator result, `You need one and a half Wood.` remained visible because `one` was authorized while `half` was ignored.
- After only `get_item(Wood)`, `Their offspring is fluffy.` remained visible in response to a breeding question because the breeding-context check only rejected unsupported reviewed entity names and this claim contained none.
- After unknown breeding calls for `Lamball + Lamball` and `Wool + Wool`, `Lamball plus Lamball produces Wool.` remained visible because entity evidence pooled arguments across unrelated breeding calls.

The temporary test command failed those four regressions with visible model answers. The tests were removed after capturing the failures; a future correction must add equivalent permanent tests before changing the gate.

Root cause: the gate authorizes fragments of numeric expressions and pools evidence by tool family rather than binding each visible calculation or breeding claim to the exact successful result and requested inputs. A list of context words cannot prove claim type, and numeric token subset matching cannot prove quantity equality.

## 2026-09-01 Third Completion Audit Correction

The four audited bypass categories were covered by permanent red regressions:

- `scaled_number_words_require_exact_calculator_values`
- `fractional_number_words_require_exact_calculator_values`
- `leading_decimal_fractions_require_exact_calculator_values`
- `generic_breeding_conclusions_require_breeding_evidence`
- `breeding_questions_reject_context_free_generic_claims`
- `breeding_evidence_is_bound_to_the_answered_parent_pair`

A positive regression, `complete_number_words_can_restate_exact_calculator_results`, also verifies that legitimate full number-word restatements remain usable.

Correction:

- Numeric parsing now treats a quantity as one complete expression, including cardinals, `hundred`/`thousand`/`million` scales, fractions such as `one and a half`, decimal literals, and fullwidth digits. It no longer authorizes one fragment of a larger expression independently.
- Numeric evidence is extracted only from JSON number leaves in successful claim-relevant records and paired with its nearest result entity, such as `item_name`, `target_name`, or the calculator query. Digits embedded in serialized version strings or identifiers are not evidence, and an unrelated entity cannot authorize a value.
- When a calculator was requested, failed calculator calls do not contribute evidence. A visible value must equal a successful result leaf and name the entity whose result contains that exact value.
- The user question is passed to the final gate. A breeding conclusion requires a successful breeding call whose parent arguments identify the pair requested by the user; arguments from other breeding calls, unrelated lookups, and unknown results cannot authorize an offspring.
- Successful breeding result IDs are mapped to reviewed canonical entity names, while unknown or ambiguous results may only restate the requested parent entities and an explicit unknown condition.
- Generic breeding prose is rejected when the user asked a breeding question or the answer claims breeding, even when it contains no reviewed entity name or breeding keyword.
- Gate violations continue to suppress the model answer, return `error`, and retain tool records, provenance, and claim-specific diagnostics.

Fresh gates after the third correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 100 tests, and `git diff --check` all passed.

## 2026-09-01 Fourth Completion Audit

The third-correction repository passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 100 tests, and `git diff --check`.

Three temporary regressions then tested the arithmetic and breeding acceptance boundary:

- After a successful calculation for 15 Wooden Clubs, `You need 75e15 Wood for Wooden Clubs.` remained visible because the parser read `75` as the complete claim and ignored the scientific exponent.
- After a successful calculation for 3 Wooden Clubs, `You need 十五 Wood.` remained visible because Chinese number words were not parsed as numeric claims.
- With only `get_item(Wood)`, `Unknown result: the offspring is fluffy.` remained visible for a Lamball breeding question because any explicit-unknown wording bypassed the missing-successful-breeding check.

The temporary test command failed those three regressions with visible model answers. The tests were removed after capturing the failures; a future correction must add equivalent permanent tests before changing the gate.

Root cause: quantity tokenization is not a complete locale-aware numeric grammar, and unknown handling is phrase-based rather than constraining the rest of the answer to permitted uncertainty language and requested parent entities.

## 2026-09-01 Fourth Completion Audit Correction

The three audited bypasses were covered by permanent red regressions:

- `scientific_notation_is_one_exact_numeric_claim`
- `chinese_number_words_require_matching_question_locale`
- `explicit_unknown_answers_cannot_append_breeding_guesses`

A positive regression, `chinese_number_words_restate_results_for_chinese_questions`, verifies that an exact Chinese numeral is accepted when the user asked in Chinese.

Correction:

- Numeric literal parsing now treats signed decimal mantissas and `e`/`E` exponents as one value. `75e15` can no longer reuse evidence for `75`.
- Numeric word parsing covers common simplified and traditional Chinese digits, scales, and decimal fractions. A Chinese number word is accepted only when the question itself uses Chinese, preventing a cross-locale numeric injection while retaining Chinese guide answers.
- Explicit unknown breeding answers are no longer authorized by the substring `unknown` alone. Their remaining words must be uncertainty connectors or the requested parent entities; qualitative offspring assertions are rejected.
- Successful breeding answers are likewise limited to breeding connectors and entities mapped from the exact successful tool result, preventing unreviewed qualitative claims after a successful call.
- Gate violations continue to suppress the model answer, return `error`, and retain tool records, provenance, and claim-specific diagnostics.

Fresh gates after the fourth correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 104 tests, and `git diff --check` all passed.

## 2026-09-01 Fifth Completion Audit

The fourth-correction repository passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 104 tests, and `git diff --check`.

Two temporary regressions then tested the arithmetic and breeding acceptance boundary:

- After a successful `calculate_shortage` for 3 Wooden Clubs with inventory Wood 5 and Wool 3 (Wood required 15, available 5, missing 10), `You are short 5 Wood and short 5 Wool for Wooden Clubs.` remained visible with `Ok` status. The value `5` is Wood's `available_quantity`, but the gate also authorized the fabricated `short 5 Wool` claim because the same sentence contains the word `Wood`.
- After an unknown `calculate_breeding_result(parent_a="Lamball", parent_b="Lamball")` (no reviewed breeding rules in the seed set), `Unknown result: the offspring is Lamball.` remained visible with `Unknown` status. The word-level allowlist strips the permitted parent phrase `Lamball` and accepts the remaining connector words, so a permitted entity in offspring-assertion position passes the round-four fix that rejected `...fluffy`.

The temporary test commands failed both regressions with visible model answers. The tests were removed after capturing the failures; a future correction must add equivalent permanent tests before changing the gate.

Root cause: a numeric claim is supported by any same-value evidence whose entity appears anywhere in the claim's sentence rather than by the entity the claim names, and the breeding unknown check is word-level, so permitted parent entities can appear in assertion position instead of only as uncertainty context.

## 2026-09-01 Fifth Completion Audit Correction

The two audited bypasses were covered by permanent red regressions:

- `same_value_evidence_cannot_authorize_another_entity_in_the_sentence`
- `unknown_breeding_answers_cannot_assert_a_permitted_offspring`

Correction:

- Numeric claims are now bound to the entity named in their immediate claim context. The gate examines the word tokens directly before and after the claim, bounded by `and`/`or`/`for`/`short`-style connectors and comma, semicolon, colon, slash, or parenthesis punctuation, and requires an exact normalized entity-name match there. A value belonging to one entity can no longer authorize a claim naming a different entity in the same sentence.
- Explicit unknown breeding answers now reject any offspring-assertion pattern (`offspring`/`child`/`result` or Chinese equivalents followed by a copula and a permitted parent entity). Permitted parent entities may appear only as uncertainty context, never as an asserted offspring.
- Gate violations continue to suppress the model answer, return `error`, and retain tool records, provenance, and claim-specific diagnostics.

Fresh gates after the fifth correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 106 tests, and `git diff --check` all passed.

## 2026-09-01 Sixth Completion Audit

The fifth-correction repository passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 106 tests, and `git diff --check`.

Three temporary regressions then tested the corrected arithmetic and breeding acceptance boundary:

- After a successful `calculate_shortage` for 3 Wooden Clubs with inventory Wood 5 and Wool 3, `You need 5 Wood, 5 Wool for Wooden Clubs.` remained visible with `Ok` status. The comma between clauses is not treated as a context boundary, so the claim-context window for the second `5` includes the preceding `5 Wood` clause and the fabricated `5 Wool` is authorized by Wood's available-quantity evidence.
- After an unknown `calculate_breeding_result(parent_a="Lamball", parent_b="Lamball")`, `Unknown result: the offspring Lamball.` remained visible with `Unknown` status. The offspring-assertion check requires a copula (`is`/`are`), so the copula-less assertion passes the word-level allowlist.
- The same unknown breeding call with `Unknown result: Lamball.` remained visible with `Unknown` status. A bare permitted entity in result position is accepted as uncertainty context even though it asserts the offspring identity.

The temporary test commands failed all three regressions with visible model answers. The tests were removed after capturing the failures; a future correction must add equivalent permanent tests before changing the gate.

Root cause: the numeric claim-context window is bounded by a connector-word list and punctuation checked only before scanned tokens, which is not a structural claim-to-entity binding; and the breeding unknown check is a copula-anchored pattern, which does not reject permitted entities in result-assertion position generally.

## 2026-09-01 Sixth Completion Audit Correction

The three audited bypasses were covered by permanent red regressions:

- `comma_separated_values_cannot_reuse_the_previous_entity`
- `unknown_breeding_answers_cannot_assert_offspring_without_a_copula`
- `unknown_breeding_answers_cannot_assert_a_bare_offspring_entity`

Correction:

- Numeric claim-to-entity binding is now a structural adjacency check: the claim context is the maximal run of tokens immediately following the number that remains a prefix of a known entity's normalized name. No connector-word list is used, so commas, conjunctions, other numbers, and sentence ends all terminate the phrase naturally, and a value cannot be reused for an entity in a later clause of the same sentence.
- Explicit unknown breeding answers now reject any permitted entity in result-assertion position: after an outcome keyword (`offspring`/`child`/`result`/`results` or Chinese equivalents), after a production verb (`produces`/`produce`/`gives`/`yields`/`hatches` and variants), with only articles and copulas allowed between. Copula-less (`Unknown result: the offspring Lamball.`) and bare (`Unknown result: Lamball.`) assertions are both rejected; permitted parent entities may appear only as uncertainty context.
- Gate violations continue to suppress the model answer, return `error`, and retain tool records, provenance, and claim-specific diagnostics.

Fresh gates after the sixth correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 109 tests, and `git diff --check` all passed.

## 2026-09-01 Seventh Completion Audit

The sixth-correction repository passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 109 tests, and `git diff --check`.

Two temporary regressions then tested the corrected breeding acceptance boundary:

- After an unknown `calculate_breeding_result(parent_a="Lamball", parent_b="Lamball")`, `Unknown: Lamball.` remained visible with `Unknown` status. The keyword-anchored assertion check had no anchor after the word `unknown`, so a bare permitted entity in result position passed as uncertainty context.
- The same unknown breeding call with `Unknown: Lamball is the offspring.` remained visible with `Unknown` status. The check only looked forward from outcome keywords and production verbs, so a permitted entity preceding the keyword (`Lamball is the offspring`) was not detected.

The temporary test commands failed both regressions with visible model answers. Reversed-clause numeric probes (`You need 5 Wool and 5 Wood for Wooden Clubs.`) confirmed the structural adjacency binding holds in both clause orders, and positive probes (`No reviewed result for Lamball and Lamball.`, `Breeding Lamball and Lamball is unknown.`) confirmed the tightened rule still accepts legitimate context-only unknown answers. A Chinese bare-entity probe (`未知：Lamball。`) was rejected as expected.

Root cause: the breeding unknown check anchored assertions on outcome keywords and production verbs, so assertions anchored only by the uncertainty marker or with the entity before the keyword escaped; the check needed to constrain permitted-entity positions structurally rather than pattern-match assertion vocabulary.

Correction:

- Explicit unknown breeding answers now allow permitted parent entities only as context. Each permitted entity occurrence must be preceded by a context marker (`for`, `of`, `between`, `with`, `and`, `plus`, `requested`, `pair`, `combination`, `matches`, `breeding`, `breed`, `from`, `to`, `parent`, `using`, and Chinese equivalents) or stand at the answer start; any other position is a result assertion and is rejected.
- The unknown-mode vocabulary is stricter than the successful-mode vocabulary: `offspring`, `child`, `produces`, `produce`, `produced`, and their Chinese equivalents are no longer allowed in explicit-unknown answers, so outcome vocabulary cannot wrap an asserted entity.
- The prior keyword-anchored check and its constants were removed; the shared permitted-phrase stripping was factored into a single helper used by both modes.

Fresh gates after the seventh correction: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 115 tests, and `git diff --check` all passed.

## Durable Uncertainty

- The canonical dataset remains a deliberately small G1 seed set, so retrieval coverage and natural-language answer breadth are intentionally limited until further reviewed intake.
- Real OpenAI-compatible and Ollama endpoints were not exercised; provider correctness is covered by offline request builders, response parsers, and the scripted mock. Live provider validation remains outstanding.
- Retrieval is lexical only. Chinese matching relies on exact alias tokenization because no semantic or language-specific vector index was added.
- The grounding gate is deterministic claim validation, not general natural-language inference. Its numeric grammar and uncertainty connector vocabulary are intentionally conservative; additional languages and answer patterns require accompanying regressions before they are allowed. The fifth, sixth, and seventh audit bypasses (same-sentence value reuse, comma-separated claim leakage, copula-less or bare offspring assertions, and uncertainty-anchored or keyword-preceded offspring assertions) now have permanent regression coverage.
- No running game, save file, server API, or dynamic player state was used, matching the Stage 1 boundary.
