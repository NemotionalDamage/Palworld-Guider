# Phase G5 Record

## Scope Delivered

- Added the loopback-only `guide-server` crate with Axum routes for health, session creation, session history, explicit user-entered snapshot import, questions, cancellation, and the minimal browser UI.
- Added bounded in-memory sessions with opaque IDs, expiry, a 32-session cap, four-exchange follow-up history, rolling ask and snapshot rate windows, and explicit missing/expired/limit errors.
- Added a 65,536-byte payload limit, JSON-depth limit, question-length validation, typed API errors, snapshot validation, and safe snapshot metadata containing only schema version, source kind, game version, freshness, and missing fields.
- Added provider-failure propagation, server deadlines, cancellation tokens, blocking-pool agent execution, bounded text-only follow-up prompts, and tests proving raw snapshots, consent data, and capture metadata do not reach provider prompts.
- Added explicit startup configuration for reviewed data, game version, loopback port, timeout, OpenAI-compatible and Ollama providers, environment-only credentials, and provider construction outside the asynchronous runtime.
- Documented local launch and configuration in `README.md`.

## Red-Green Evidence

- Session state first failed because `GuideServer` session types did not exist; seven session tests then passed.
- Web routing first failed because `GuideServer` was absent; eleven API tests then passed after routes, typed errors, validation, rate limits, snapshot metadata, and local UI were added.
- Runtime tests first failed on provider-script exhaustion behavior, an unknown cancellation endpoint, and a synchronous provider deadline. Five runtime tests then passed after moving agent execution to the blocking pool, adding the cancellation registry, and enforcing the server deadline.
- A cancellation-specific regression first returned `session_not_found`; it then returned the dedicated `cancellation_not_found` envelope.
- CLI tests first failed because the `guide-server` binary target did not exist; six CLI tests then passed after explicit parsing, environment-only provider setup, loopback enforcement, and safe provider construction were added.

## Acceptance Mapping

- Web grounded answers: the API reuses `GuideAgent` and returns its complete answer envelope, including tool calls, provenance, version, uncertainty, and errors.
- Session and context: bounded history and follow-up behavior are tested.
- Provider configuration: provider kind, model, optional base URL, and OpenAI key are environment-only; the key is not echoed.
- Safety controls: payload limits, JSON depth, question bounds, rate limits, session expiry, provider failure, server timeout, cancellation, and redaction have dedicated tests.
- Browser UI: a dependency-free local page exposes question, snapshot, provenance, uncertainty, history, and error regions without external origins.
- Optional adapters: Tauri, Discord, QQ, private-server REST, UE4SS, and in-game chat are intentionally absent.
- Mutation boundary: no movement, combat, gathering, construction, inventory mutation, world mutation, save writer, arbitrary shell command, or game mutation path exists.

## Verification

Fresh final gates:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` with 166 tests
- `git diff --check`

All passed for the completed Web milestone. Provider validation remains offline through scripted and deterministic test providers.

## Durable Uncertainty And Blockers

G5 is not complete. The final private local read-only in-game chat validation cannot begin until the project owner records:

- exact Palworld version
- platform and installation/load mode
- save backup location and retention choice
- ownership and explicit consent scope
- selected adapter path
- private-server endpoint/object constraints, if applicable
- UE4SS target-build-verified fields, if applicable

No runtime game-process access, save reader, private-server REST client, UE4SS adapter, or in-game chat adapter is enabled. Live OpenAI-compatible and Ollama endpoints and the real browser flow were not exercised in this milestone.

## Read-Only Local Target Audit

On 2026-09-01, a filesystem-only audit identified the local game as Steam Windows Palworld App `1623730`, installed build `24575825`. The offline manifest did not expose a semantic game-version label, so the immutable Steam build ID is the current exact-build identity.

UE4SS `3.0.1 Beta #0` (Git SHA `d935b5b`) is installed and its existing log shows successful loading against the shipping Windows executable and Unreal Engine `5.1`. This does not verify any Palworld Guider field. The historical log also showed the older project's `PalAgentPhase1` mod enabled with unavailable websocket transport configuration. On 2026-09-01, while Palworld was not running, that mod directory and its load entry were removed while preserving UE4SS and the disabled `PalAgentPhase0`, `PalAgentStageB`, and shared directories. Palworld was not relaunched after the removal, so a fresh UE4SS load still needs validation.

Local save data and backup directories exist. No game process was launched, no save file was read, and no game state was accessed. The project owner still needs to approve the ownership and consent scope, choose backup retention, select UE4SS versus another thin adapter path, and define the exact-build field allowlist.

### Approved Target Decisions

The project owner approved the UE4SS local-client path on 2026-09-01. The live validation target is the local single-player Steam client. The owner consented to capture `!guide ` chat input, send system-chat replies, and read both target-build-verified field families: player position and active-Otomo identity/position. No other client fields are approved. The validation world is `3C2BA10146F65256FD1B889FBF5F854F`. Before adapter installation, its complete save directory must be copied to `.local/backups/g5/3C2BA10146F65256FD1B889FBF5F854F/` and retained for 30 days.

The approved UE4SS design is recorded in `docs/superpowers/specs/2026-09-01-phase-g5-ue4ss-design.md` and its nine-task, test-first implementation plan in `docs/superpowers/plans/2026-09-01-phase-g5-ue4ss.md`. No adapter code has been written and no runtime integration has begun.
