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
- `recipe`: output, ingredients, optional by-products, crafting stations, optional technology, and optional crafting time
- `pal`: names, optional stats, work suitability, drops, and habitat references
- `habitat`: names and referenced Pals
- `breeding_rule`: parent A, parent B, and child Pal references
- `alias`: locale-specific alias and target fact
- `progression_relationship`: typed `unlocks`, `requires`, or `improves` edge
- `conflict`: subject, field, competing values, evidence sources, and resolution state

## Validation Rules

- IDs are nonempty, contain at least one ASCII letter or digit, and use only ASCII letters, digits, hyphen, or underscore.
- English names are required; localized names cannot be blank when present.
- `applicable_game_version` uses dot-separated numeric segments with at least two segments, for both sources and fact provenance.
- Alias locales are restricted to `en` and `zh_hans`.
- Dates use a valid `YYYY-MM-DD` Gregorian calendar date; evidence URLs use HTTP(S) or a reviewed `local://` path.
- Fact IDs are globally unique, and source IDs are unique.
- Recipes require at least one ingredient and one crafting station.
- Recipe ingredient, output, by-product, and technology references resolve to the exact required record type.
- Recipe and output quantities are greater than zero; present crafting time is finite and greater than zero.
- Work-suitability levels are 1–5.
- Drop quantities are positive, minimum does not exceed maximum, and probability is 0–100.
- Item, Pal, recipe, technology, habitat, breeding, and source references resolve to their required record types. Generic alias targets, progression endpoints, and conflict subjects may reference any fact.
- Conflicts require at least two nonempty competing values and at least one registered evidence source.
- Conflicts remain records and are never silently resolved by loading.
