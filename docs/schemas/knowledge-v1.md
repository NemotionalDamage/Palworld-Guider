# Knowledge Schema v1

Knowledge v1 is the canonical reviewed-record format. Canonical data is stored as one JSON object per line in:

- `data/reviewed/sources.jsonl`
- `data/reviewed/facts.jsonl` for the legacy mixed seed set
- classified fact files such as `data/reviewed/items.jsonl` and `data/reviewed/aliases.jsonl`

## Source Records

Source records register the evidence boundary before game facts are normalized. They contain:

- stable source ID, title, and supplier
- retrieval date and evidence URLs
- claimed applicable Palworld version
- reviewer, review status, and confidence
- review notes

## Fact Envelope

Every fact record carries an inline provenance object:

- `source_id`
- `applicable_game_version`
- `retrieved_on`
- `reviewer`
- `review_status`
- `confidence`
- optional `change_risk`

Validation requires the source to exist, the fact to be reviewed, the confidence to support normal answers, and version/date/reviewer/status/confidence fields to agree with the registered source.

## Fact Types

- `item`: localized names, description, rarity, and acquisition leads
- `technology`: localized names and technology level
- `recipe`: output, ingredients, optional by-products, crafting stations, optional technology, and the optional unlock item
- `pal`: names, optional stats, work suitability, drops, habitat references, optional habitat leads, an optional reviewed wild-spawn verdict, and optional breeding ranks
- `habitat`: names and referenced Pals
- `map_definition`: the logical map, its world bounds, logical size, and priority
- `map_region`: localized area name plus an optional reviewed boundary volume and its `boundary_is_reviewed` flag
- `map_point`: fast-travel or boss-tower point with world coordinates
- `breeding_rule`: parent A, parent B, and child Pal references
- `alias`: locale-specific alias and target fact
- `progression_relationship`: typed `unlocks`, `requires`, or `improves` edge
- `conflict`: subject, field, competing values, evidence sources, and resolution state

## Name Resolution

- A query that matches exactly one record resolves to it.
- Several records may share one localized display name. Rank variants of one item share a name and differ only by `rarity`, so the base (lowest-rarity) tier answers a bare name and a caller reaches another tier by asking for that rarity.
- A family whose members all sit at one rarity stays ambiguous, so the answer never guesses.
- The recipe of a schematic-locked tier carries an `unlock_item_id` (the target build's `UnlockItemID`), the item that must be in inventory before the recipe can be used. The answer then names that schematic: a locked tier points at its own schematic, and the base tier of a schematic family points at the higher tiers. An item that ships as a single tier without any sibling still names the schematic that unlocks it. Families that are found in the world or unlocked by technology (Pal eggs, treasure maps, Grappling Gun and Meteor Launcher tiers) carry no schematic claim.
- A recipe that produces an item without naming an unlock item is treated as craftable on its own, so the answer makes no schematic claim for that tier. Only an item with no producing recipe row in the dataset falls back to the shipped `Blueprint_<native_row_id>` name match.

## Breeding Resolution

Only explicit `breeding_rule` exceptions are stored. The general case is computed, so the dataset never holds the full parent-pair cross product.

`pal` records carry the fields that drive the formula:

- `breeding_combi_rank`: the Pal's combi rank, used both as a formula input and as its position among candidate children.
- `breeding_combi_priority`: tie-breaker when two candidate children sit equally far from the target rank; the higher value wins.
- `breeding_ignore_combi`: the Pal is not eligible as a formula child.
- `breeding_self_only`: the Pal is only obtainable by pairing two of its own species.
- A rank of `0` or `9999`, or no rank at all, marks a Pal without a usable combi rank.
- A Pal the target build gives no rank may declare `breeding_combi_rank` and `breeding_combi_priority` in `local_evidence.reviewed_empty_fields` when the owner reviewed why; a Pal that only breeds with its own species also carries an explicit self combination in `breeding_rules.jsonl`.

`guide-core` resolves a parent pair in this order:

1. A matching explicit `breeding_rule` wins.
2. Two parents of the same species return that species.
3. Otherwise the target rank is `floor((rankA + rankB + 1) / 2)`, and the eligible child with the smallest rank distance wins. Ties break on higher `breeding_combi_priority`, then on ID.
4. A Pal is eligible as a formula child only when it has a usable rank, is not `breeding_ignore_combi`, and is not the child of an explicit rule.

Formula results report the pseudo rule ID `BREEDING_FORMULA`; explicit results report the matching `breeding_rule` ID.

## Validation Rules

- IDs are nonempty, contain at least one ASCII letter or digit, and use only ASCII letters, digits, hyphen, or underscore.
- English names are required; localized names cannot be blank when present.
- `applicable_game_version` uses dot-separated numeric segments with at least two segments, for both sources and fact provenance.
- Alias locales are restricted to `en` and `zh_hans`.
- Dates use a valid `YYYY-MM-DD` Gregorian calendar date; evidence URLs use HTTP(S) or a reviewed `local://` path.
- Fact IDs are globally unique, and source IDs are unique.
- Recipes require at least one ingredient and one crafting station.
- Recipe ingredient, output, by-product, and technology references resolve to the exact required record type.
- Recipe and output quantities are greater than zero.
- Work-suitability levels are 1–5.
- Drop quantities are positive, minimum does not exceed maximum, and probability is 0–100.
- Item, Pal, recipe, technology, habitat, breeding, and source references resolve to their required record types. Generic alias targets, progression endpoints, and conflict subjects may reference any fact.
- Conflicts require at least two nonempty competing values and at least one registered evidence source.
- Conflicts remain records and are never silently resolved by loading.
