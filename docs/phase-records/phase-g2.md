# Phase G2 Record

## Scope

Phase G2 added the deterministic offline guide core:

- `guide-core` library and JSON CLI binary
- exact ID resolution and normalized English-name matching
- reviewed alias resolution
- ambiguous-name and unknown-name handling
- item, Pal, technology, and recipe lookup
- provenance summaries and configured-game-version mismatch warnings
- recursive recipe material trees
- multiple-output batch scaling
- duplicate raw-ingredient aggregation
- by-product totals
- inventory shortage calculation
- additional craftable-count calculation with dependency tree
- checked arithmetic, cycle detection, depth limits, and alternative-recipe ambiguity
- order-insensitive breeding-result lookup
- bounded shortest breeding-chain traversal
- commands: `lookup`, `recipe`, `materials`, `shortage`, `craftable`, `breeding`, and `chain`

No LLM, retrieval index, provider, adapter, server, save reader, or dynamic-state source was added.

## Deterministic Behavior

- Exact IDs take precedence over normalized names.
- Names and aliases are normalized to ASCII letters and digits with case folded.
- Multiple matching entities or recipes return `ambiguous`; no winner is silently selected.
- Missing reviewed records return `unknown`.
- Alternative recipes remain explicit ambiguity.
- Recipe cycles and depth-limit violations return `error`.
- Quantities use checked `u32` arithmetic.
- Raw acquisition targets are not reported as craftable.
- Craftable counts describe additional items and include the full material calculation.
- Breeding rules match either parent order.
- Breeding chains use bounded traversal and retain shortest-path ambiguity.

## Verification

Red test checkpoints:

- Resolution and lookup initially failed because `GuideEngine` and `AnswerStatus` did not exist.
- Calculator tests initially failed because calculator APIs and by-product schema did not exist.
- Breeding tests initially failed because breeding APIs did not exist.
- CLI tests initially failed because no `guide-core` binary target existed.
- A craftable-tree regression initially failed because the result omitted the dependency calculation.
- A raw-target craftable regression initially failed by reporting an item without a recipe as craftable.

Final gates:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed with 23 tests.

No checks were intentionally skipped.

## Durable Uncertainty

- The canonical dataset remains a deliberately small G1 seed set; broad game coverage is a continuing knowledge-intake task, not a calculator-correctness claim.
- No canonical breeding rule has been reviewed, so canonical breeding commands return `unknown`.
- Paldb remains a single reviewed-secondary source and each fact carries patch-change risk.
- Version comparison requires an explicitly configured game version; no game process or save detection exists.
- The CLI returns deterministic JSON rather than natural-language prose; natural-language synthesis is Phase G3.
