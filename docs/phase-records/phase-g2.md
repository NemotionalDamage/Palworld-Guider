# Phase G2 Record

## Scope

Phase G2 added the deterministic offline guide core:

- `guide-core` library and JSON CLI binary
- exact ID resolution, normalized English-name matching, and Unicode alias support
- ambiguous-name and unknown-name handling
- item, Pal, technology, and recipe lookup
- provenance summaries and configured-game-version mismatch warnings
- recursive recipe material trees
- multiple-output batch scaling
- duplicate raw-ingredient aggregation
- by-product totals
- inventory shortage calculation with intermediate-inventory consumption
- additional craftable-count calculation with dependency-tree inventory consumption
- checked arithmetic, cycle detection, depth limits, and alternative-recipe ambiguity
- byproduct item provenance and conflict propagation into material calculations
- byproduct acquisition relationships in item lookup
- same-item and cross-item byproduct offsets in shortage and craftable calculations
- order-independent cross-item byproduct offsets via producer-before-consumer child ordering
- order-insensitive breeding-result lookup
- bounded shortest breeding-chain traversal
- explicit ambiguous breeding parent and endpoint candidates
- related recipe, Pal, habitat, ingredient, byproduct, and technology conflict propagation for all lookups
- canonical snake_case provenance summary serialization
- commands: `lookup`, `recipe`, `materials`, `shortage`, `craftable`, `breeding`, and `chain`

No LLM, retrieval index, provider, adapter, server, save reader, or dynamic-state source was added.

## Deterministic Behavior

- Exact IDs take precedence over normalized names.
- Names and aliases retain Unicode letters and digits with case folded; empty normalized queries return `unknown`.
- Multiple matching entities or recipes return `ambiguous`; no winner is silently selected.
- Missing reviewed records return `unknown`.
- Alternative recipes remain explicit ambiguity.
- Recipe cycles and depth-limit violations return `error`.
- Quantities use checked `u32` arithmetic.
- Raw acquisition targets are not reported as craftable.
- Shortage calculations consume stocked target and intermediate items, credit same-item and cross-item byproducts, and then expand remaining missing quantities into raw-material shortages; sibling subtrees are processed producer-before-consumer so credits apply regardless of ingredient declaration order.
- Craftable counts describe additional items, consume intermediate inventory through the dependency tree, credit recipe byproducts with the same order-independent sibling processing, report boundary-limiting materials, and reject representational overflow.
- Every lookup propagates conflicts from records actually included in its relation results, including byproduct and habitat relationships.
- Provenance review status and confidence use the same snake_case values as canonical records.
- Breeding rules match either parent order.
- Breeding chains use bounded traversal and retain shortest-path ambiguity.
- Ambiguous parent or endpoint names retain candidate IDs and return `ambiguous` rather than `unknown`.

## Verification

Red test checkpoints:

- Resolution and lookup initially failed because `GuideEngine` and `AnswerStatus` did not exist.
- Calculator tests initially failed because calculator APIs and by-product schema did not exist.
- Breeding tests initially failed because breeding APIs did not exist.
- CLI tests initially failed because no `guide-core` binary target existed.
- A craftable-tree regression initially failed because the result omitted the dependency calculation.
- A raw-target craftable regression initially failed by reporting an item without a recipe as craftable.
- A multi-output craftable regression initially failed by returning `1` item for a one-batch recipe yielding `5`.
- A craftable overflow regression initially returned `ok` instead of an arithmetic error.
- Ambiguous breeding parent and endpoint regressions initially returned `unknown` instead of `ambiguous` with candidate IDs.
- Intermediate-inventory shortage and craftable regressions initially reported a missing raw material and zero craftable output.
- A Unicode-alias regression initially matched a punctuation-only query because both normalized to an empty string.
- An item relation regression initially omitted unresolved recipe and Pal conflicts.
- A provenance envelope regression initially rendered `reviewed_secondary` as `reviewedsecondary`.
- A byproduct provenance regression initially omitted the byproduct item source and reported a single knowledge version.
- An item lookup regression initially omitted byproduct acquisition relationships.
- A shortage and craftable regression initially ignored byproduct credits and over-required raw materials.
- A related-conflict regression initially propagated conflicts only for item lookups, not Pal, technology, or recipe lookups.
- An order-dependence regression initially reported an extra Fiber shortage and zero craftable count when the consuming ingredient was declared before the producing sibling; both ingredient orders now agree.

Final gates:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed with 38 tests.

No checks were intentionally skipped.

## Durable Uncertainty

- The canonical dataset remains a deliberately small G1 seed set; broad game coverage is a continuing knowledge-intake task, not a calculator-correctness claim.
- No canonical breeding rule has been reviewed, so canonical breeding commands return `unknown`.
- Paldb remains a single reviewed-secondary source and each fact carries patch-change risk.
- Version comparison requires an explicitly configured game version; no game process or save detection exists.
- The CLI returns deterministic JSON rather than natural-language prose; natural-language synthesis is Phase G3.
