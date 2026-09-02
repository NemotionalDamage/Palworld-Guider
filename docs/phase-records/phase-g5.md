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

The approved UE4SS design is recorded in `docs/superpowers/specs/2026-09-01-phase-g5-ue4ss-design.md` and its nine-task, test-first implementation plan in `docs/superpowers/plans/2026-09-01-phase-g5-ue4ss.md`. The UE4SS adapter path is now implemented and passed its offline acceptance gate; only approved live validation remains.

## UE4SS Implementation Scope

The approved UE4SS local-client path is implemented offline:

- `crates/game-gateway`: authenticated schema-2 loopback WebSocket gateway with 65,536-byte frames, 64-message queue caps, three-second default tool timeout, strict sequences, one active session with replacement and cancellation.
- `crates/guide-tools` + `crates/guide-agent`: `RuntimeToolSource` composition (dynamic tool visibility, no static shadowing, shared call budget) and a narrow x/y/z numeric grounding carve-out for the two approved observation tools.
- `crates/guide-adapter`: `GameAdapterRuntime` (allowlist intersection, fail-closed dispatch, internal-only `send_chat_message`) and `InGameChatBridge` (exact `!guide ` prefix, ping bypass, four-exchange bounded text-only history, six asks/minute, 1000/400 character bounds).
- `crates/guide-server`: `--adapter-port`/`--adapter-token-env` adapter mode with environment-only token validation, loopback-only binding, and a dedicated serial event service sharing one agent with the Web server.
- `adapter/read-only/ue4ss`: renamed transport-only native shim (`PalworldGuider`, callbacks `guider_cfg`/`guider_conn`/`guider_send`/`guider_poll`/`guider_stat`/`guider_close`, exports `start_mod`/`uninstall_mod`) plus Lua adapter (`ping`, `get_player_status`, `get_active_pal_status`, `send_chat_message`; guarded chat hook; finite-position and active-Otomo reads; no file IPC).
- `scripts/`: `Build-G5Ue4ss.ps1`, `Backup-G5Save.ps1`, `Install-G5Ue4ss.ps1`, `Uninstall-G5Ue4ss.ps1` with SHA256 verification, backup-before-install ordering, save backup with 30-day retention, and fail-closed guards.

## Offline Acceptance Gate Results

Fresh final gates (2026-09-01):

- `cargo fmt --all -- --check` clean.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` with 250 tests, all passing.
- `git diff --check` clean.

Native package built with `scripts/Build-G5Ue4ss.ps1 -Ue4ssDll <UE4SS_v3.0.1\UE4SS.dll>`; SHA256 manifest verified. `dumpbin` shows `main.dll` exports exactly `start_mod` and `uninstall_mod` with dependents `UE4SS.dll`, `WINHTTP.dll`, and MSVC runtime only. Build-time package hashes: `dlls/main.dll` is non-reproducible across MSVC builds (observed `fa50e3f9...`); `Scripts/main.lua` `92263140c893c103cd4b10cfd6d7b9a38c7d762ea4d205d6ea4f7c16254e1258`, `Scripts/pal_transport.lua` `0be84de77e0b2ef42fb83ef3e27da2a1a1693c4265d8aadd33fd83ac8a45f2bb`, `Scripts/pal_json.lua` `56b16dac3cd2300795b4730460f49d18570fbac7a09ab6c390977680ac69a698`.

Offline loopback rehearsal passed: `guide-server --data data\reviewed --adapter-port 8071` with a process-only `PALWORLD_GUIDER_GATEWAY_TOKEN`; a fake schema-2 adapter client authenticated, sent hello/manifest, sent `!guide ping`, received exactly one `send_chat_message` tool call with the exact reply `Pong: Palworld Guider adapter connected.`, and answered it. The provider was never called (ping bypass), and no `.local` transcript contains player text or the reply text. Palworld was not launched and no save, game directory, or live game state was touched.

## Remaining Live-Only Blockers

G5 is not yet complete. The remaining acceptance criteria require approved live validation against the recorded local single-player client (build `24575825`, UE4SS `3.0.1 Beta #0`):

- Back up world `3C2BA10146F65256FD1B889FBF5F854F` before adapter installation, verify the backup, then install the hash-verified package.
- Launch Palworld and validate `!guide ping`, a factual question, a player-position question, and an active-Otomo question with an Otomo spawned.
- Restart and fail-closed checks (server restart, game restart, server-down recovery) and a clean-exit check.
- These steps require explicit project-owner approval to launch the game.

## G5 Grounding Fix

### Scope Delivered

- Added a Rust `FactSheet` that derives stable quantity, entity, observation, and version slots from successful deterministic tool results and reviewed tool arguments.
- Added the model-visible `submit_answer` termination tool. It is not dispatched through the tool registry and does not consume the deterministic tool-call budget.
- Added strict draft rendering: slot references must be declared and exist; digits and English/Chinese number words are forbidden in model text; known entities require tool evidence; Rust fills numeric values and renderer-added step numbers.
- Added deterministic fallback rendering for invalid drafts, free-text responses, and tool-budget exhaustion, so grounding failures no longer produce a user-visible rejection.
- Preserved a strict compatibility path for existing text-only providers: numeric-free and entity-safe free text is rendered by the same Rust validator; otherwise it falls back.
- Removed the old natural-language numeric grounding heuristics and temporary diagnostics.

### Verification

Fresh final gates on 2026-09-02:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `git diff --check`

All passed. The focused `guide-agent` suite contains 30 tests covering quantity tampering, unknown slots, numeric literals, English and Chinese number words, renderer numbering, exact observations, unsupported entities, free-text compatibility/fallback, provenance, versions, budgets, cancellation, timeout, and truncation.

No game was launched and no live adapter validation was performed. The required live replay of the Wood, Food, and Lamball-spawn questions remains part of the existing G5 live-only gate and requires explicit project-owner approval to launch Palworld.
