# Knowledge Source Policy

Game facts must be reviewed, versioned, and traceable. The language model is not a source of truth.

## Source Priority

1. **Local target-build data** — extracted from the exact installed Palworld version.
2. **Official sources** — official patch notes or official documentation.
3. **User-reviewed secondary sources** — supplied or selected by the project owner and reviewed before use.
4. **Community sources** — wikis and databases, usable only as leads or supporting references.
5. **Model-derived hypotheses** — never persisted as facts and never presented as evidence.

Lower-priority sources may identify a question or conflict, but cannot silently override a higher-priority source.

## Required Provenance

Every structured fact must be associated with:

- applicable Palworld version
- source identifier
- source URL or local extraction path
- retrieval or extraction date
- reviewer
- review status
- confidence
- notes or known conflicts

## Confidence Levels

- `verified-target`: extracted from or reproduced against the exact target build.
- `official`: supported by an official source.
- `reviewed-secondary`: user-reviewed but not officially confirmed.
- `community`: unreviewed or partially reviewed community information.
- `conflicted`: sources disagree.
- `unknown`: insufficient evidence.

Only `verified-target`, `official`, and `reviewed-secondary` records may support normal guide answers. Community records require an explicit caveat. Conflicted and unknown records must not be presented as settled facts.

## Intake Rules

- Register every useful source in `docs/reference-data/source-log.md` before using it.
- Record who supplied it, when it was retrieved, and which game version it claims to cover.
- Normalize only the required facts into structured data.
- Do not copy game assets, large tables without transformation, or long passages of copyrighted text.
- Preserve a brief source summary rather than a wholesale page dump.
- Record conflicting values as conflicts; do not choose a winner without review.
- Add a regression case when a source correction changes a user-visible answer.

## Version Compatibility

Palworld updates can change items, recipes, stats, drop rates, Pal behavior, and progression requirements.

The future runtime must compare:

- knowledge base version
- applicable game version range
- detected or configured current game version

When they do not match, answers must warn that recommendations may be stale.
