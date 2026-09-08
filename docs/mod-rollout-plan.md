# Palworld Guider MOD Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a Level 1 clone-to-game source path and a Level 2 downloadable installer path for the read-only UE4SS MOD and local guide service.

**Architecture:** The game process contains only the existing Lua/C++ transport MOD. The Rust `guide-server` remains a separate loopback-only process and owns knowledge retrieval, provider calls, grounding, and answer generation. Distribution tooling verifies the Palworld build, UE4SS DLL, package hashes, and save backup before writing to the game directory.

**Tech Stack:** Rust 1.98+, PowerShell 5.1+, CMake/Ninja with VS 2022 Build Tools, UE4SS 3.0.1, and GitHub Releases.

**Spec:** This document is the active rollout specification.

## Global Constraints

- Windows 10/11 x64 only.
- The guide server and adapter gateway bind only `127.0.0.1`.
- The MOD remains read-only: player position, active-Otomo identity/position, chat input, and chat reply only.
- No provider API key or gateway token may be written to a file, printed, or committed.
- Installation must refuse unsupported Palworld builds and UE4SS DLL hashes.
- Installation must require a verified save backup before writing under the game directory.
- Installation, update, and uninstall must preserve all unrelated mods.
- Current reviewed knowledge targets Palworld `1.0.3` / Steam build `24575825`.
- Build `25094871` is blocked until a three-layer compatibility probe passes.
- Run targeted tests during development. Do not run the full workspace suite unless a level gate explicitly requires it.
- At the end of every Level, create one coherent commit and push it to both configured remotes.

## Level Completion And Release Policy

| Level | Deliverable | Required commit | Push |
|---|---|---|---|
| Level 0 | Repository focused on MOD rollout; obsolete contracts, local secrets, backups, and raw research removed; this plan added | `chore: focus repository on mod rollout` | `git push origin main` and `git push tsinghua main` |
| Level 1 | A player can clone the repository, build in release mode, run one setup command, run one start command, and use `!guide ping` in a supported game | `feat: enable source-built in-game mod` | Both remotes |
| Level 2 | A player can download a release ZIP, run its installer, configure a provider, start the service, and use `!guide ping` without Rust or VS Build Tools | `feat: add mod installer release package` | Both remotes |

Do not start a later level before the active level acceptance gate passes. If a live compatibility probe fails, stop and record the failing layer in this plan before changing code.

---

## Level 1: Clone-To-Game Source Path

### Task 1: Support Matrix And Preflight

**Files:**

- Create: `adapter/read-only/ue4ss/support-manifest.json`
- Create: `scripts/Test-InGameGuide.ps1`
- Modify: `scripts/Build-Ue4ssAdapter.ps1`
- Test: `crates/guide-server/tests/adapter_scripts.rs`

**Interfaces:**

- `support-manifest.json` contains `schema_version`, `game_build_ids`, `game_version`, `ue4ss_version`, `ue4ss_dll_sha256`, `member_variable_layout_sha256`, `package_files`, and `blocked_game_build_ids`.
- `Test-InGameGuide.ps1` accepts `-GameRoot`, `-Ue4ssDll`, and `-SaveDirectory`; omitted paths are discovered automatically.
- `Build-Ue4ssAdapter.ps1` continues to accept `-Ue4ssDll` and `-OutputDirectory`.

**Implementation steps:**

- [ ] Create the support manifest with `24575825` allowed and `25094871` blocked.
- [ ] Discover Steam libraries through `libraryfolders.vdf` and read `appmanifest_2379380.acf`.
- [ ] Discover the newest save containing `Level.sav` under `%LOCALAPPDATA%\Pal\Saved\SaveGames`.
- [ ] Verify the game build, UE4SS DLL hash, `Mods` directory, and support manifest.
- [ ] Replace fixed VS Build Tools paths with `vswhere` discovery and explicit failure guidance.
- [ ] Add source-contract tests that assert the support manifest contains both build IDs and preflight contains Steam manifest, save, and hash checks but no `Copy-Item` or `Remove-Item`.
- [ ] Run `cargo test -p guide-server --test adapter_scripts`.
- [ ] Run `scripts/Test-InGameGuide.ps1` on the development machine; record only the exit result, not a live-game transcript.

### Task 2: One-Command Setup And Start

**Files:**

- Create: `scripts/Setup-InGameGuide.ps1`
- Create: `scripts/Start-InGameGuide.ps1`
- Modify: `scripts/Backup-PalworldSave.ps1`
- Modify: `scripts/Install-Ue4ssAdapter.ps1`
- Test: `crates/guide-server/tests/adapter_scripts.rs`

**Interfaces:**

- `Setup-InGameGuide.ps1` accepts optional `-GameRoot`, `-Ue4ssDll`, `-SaveDirectory`, and `-OutputDirectory`.
- `Start-InGameGuide.ps1` accepts optional `-Port`, `-AdapterPort`, and `-ServerExecutable`.
- Setup creates only `.local/build/...` and `.local/backups/...` inside the repository.
- Start generates a random token with `[System.Guid]::NewGuid().ToString('N') * 2`.

**Implementation steps:**

- [ ] Make setup invoke preflight, backup, build, hash verification, and install in that order.
- [ ] Support `-WhatIf` for preflight and build staging; refuse `-WhatIf` for real game installation because the install script must remain explicit.
- [ ] Refuse to overwrite an existing `Mods/PalworldGuider`; direct users to `Uninstall-Ue4ssAdapter.ps1`.
- [ ] Make start validate the installed package manifest, generate an in-memory token, pass through provider environment variables, and launch `guide-server`.
- [ ] Never prompt for or store a provider API key.
- [ ] Add source-contract tests proving setup composes the four safe scripts and start never writes the token to a file.
- [ ] Run `cargo test -p guide-server --test adapter_scripts`.
- [ ] Run setup with `-WhatIf`, then without `-WhatIf` on a supported development build.

### Task 3: Current-Build Compatibility Probe

**Files:**

- Modify: `adapter/read-only/ue4ss/Scripts/main.lua`
- Modify: `adapter/read-only/ue4ss/support-manifest.json`
- Test: `crates/guide-adapter/tests/lua_source.rs`

**Implementation steps:**

- [x] Add a compile-time Lua capability switch with exactly these values: `noop`, `chat`, and `full`.
- [x] Keep `full` as the default. `noop` supports only transport heartbeat and `ping`; `chat` disables player and Otomo reads.
- [x] Add Lua source tests proving every switch value is recognized and no mutation API is introduced.
- [x] On build `25094871`, test in this exact order (live probe result below):
  1. UE4SS no-op MOD for one game launch and save entry.
  2. Chat-only MOD with `!guide ping`.
  3. Full MOD with `!guide ping`, one factual question, and one nearest-waypoint question.
- [x] A layer passes only if the game remains stable and the expected response is returned.
- [x] Add `25094871` to `game_build_ids` only after all three layers pass; otherwise leave it blocked and record the first failing layer in this plan.
- [x] Run `cargo test -p guide-adapter --test lua_source`.

**Live probe result (2026-09-08, build `25094871`, machine with Steam save
`3C2BA10146F65256FD1B889FBF5F854F`):** Layers 1 (noop) and 2 (chat)
PASSED; layer 3 (full) was not run, so `25094871` remains blocked and is
NOT added to `game_build_ids`.

Initial failure and resolution - the first live attempt crashed with heap
corruption (`0xc0000374`, faulting module `ntdll.dll`) when entering the
save with UE4SS 3.0.1 enabled. Control runs attributed the crash to UE4SS,
not the Guider adapter: it reproduced with `PalworldGuider` disabled and
disappeared with UE4SS disabled. The crash was then resolved on this
machine by disabling UE4SS 3.0.1's four world-load hooks in
`UE4SS-settings.ini` (see the manual-start flow in `README.md`):

- `HookInitGameState = 0`, `HookCallFunctionByNameWithArguments = 0`,
  `HookBeginPlay = 0`, `HookLocalPlayerExec = 0`
- Keep `HookProcessInternal = 1` and `HookProcessLocalScriptFunction = 1`;
  optionally set `GuiConsoleEnabled = 0`

After the fix on build `25094871`:

1. Layer 1 (noop) PASSED: the save loads and stays stable; the server log
   shows the adapter hello, the 1-tool capability manifest, and continuous
   heartbeat frames accepted by the gateway.
2. Layer 2 (chat) PASSED: `!guide ping` in the chat box returns
   `Pong: Palworld Guider adapter connected.`; the server log shows the
   event frame, a `send_chat_message` tool call, and the tool result
   accepted by the gateway.
3. Layer 3 (full) NOT RUN: it needs a live player to answer one factual
   and one nearest-waypoint question in chat. Until it passes, `25094871`
   stays blocked; follow the manual-start flow in `README.md`, then
   re-probe with the `full` capability build before unblocking.


### Task 4: Clone-To-Game README

**Files:**

- Modify: `README.md`
- Modify: `docs/deployment.md`
- Modify: `adapter/read-only/ue4ss/README.md`

**Implementation steps:**

- [ ] Put the in-game MOD path before Web-only instructions.
- [ ] Document prerequisites: supported Palworld build, UE4SS 3.0.1, Rust 1.98+, VS 2022 Build Tools with CMake/Ninja, and provider environment variables.
- [ ] Document this complete path:

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

- [ ] In game, document only `!guide ping` and `!guide <question>`.
- [ ] Document uninstall and supported-build failure behavior.
- [ ] Remove machine-specific examples and obsolete references.

### Level 1 Acceptance Gate

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy -p guide-server -p guide-adapter --all-targets -- -D warnings`
- [ ] `cargo test -p guide-server --test adapter_scripts`
- [ ] `cargo test -p guide-adapter --test lua_source`
- [ ] A clean clone builds with `cargo build --release`.
- [ ] Setup succeeds on a supported build and refuses an unsupported build.
- [ ] Start prints the loopback Web and adapter addresses without printing the token.
- [ ] `!guide ping` returns the fixed pong message without calling the provider.
- [ ] One factual question and one nearest-waypoint question return useful, grounded replies.
- [ ] Uninstall removes only `Mods/PalworldGuider` and its `mods.txt` entry.
- [ ] Commit and push `feat: enable source-built in-game mod` to both remotes.

---

## Level 2: Downloadable Installer Release

### Task 5: Release Package Builder

**Files:**

- Create: `scripts/New-ModRelease.ps1`
- Create: `scripts/install-client.ps1`
- Create: `scripts/start-guide.ps1`
- Create: `scripts/uninstall-client.ps1`
- Create: `.github/workflows/mod-release.yml`
- Test: `crates/guide-server/tests/adapter_scripts.rs`

**Implementation steps:**

- [ ] Build `guide-server.exe` and the UE4SS MOD package in a clean release environment.
- [ ] Download pinned UE4SS `v3.0.1`, verify its ZIP and DLL hashes, and use it only as a build input.
- [ ] Produce `PalworldGuider-Setup-<version>-win-x64.zip` containing `install-client.ps1`, `start-guide.ps1`, `uninstall-client.ps1`, `guide-server.exe`, `data/reviewed/`, `PalworldGuider/`, `support-manifest.json`, `SHA256SUMS.txt`, and `README.txt`.
- [ ] Do not code-sign yet, but require users to verify `SHA256SUMS.txt` before installation.
- [ ] Add source-contract tests proving release packaging includes the manifest and excludes `.env`, `.local`, Git metadata, raw research data, and target directories.
- [ ] Run `cargo test -p guide-server --test adapter_scripts`.

### Task 6: No-Toolchain Installer

**Files:**

- Modify: `scripts/install-client.ps1`
- Modify: `scripts/start-guide.ps1`
- Modify: `scripts/uninstall-client.ps1`
- Modify: `README.md`
- Modify: `docs/deployment.md`

**Implementation steps:**

- [ ] Verify the release hash manifest before any write.
- [ ] Locate the game, check the support manifest, verify UE4SS, discover or prompt for the save directory, create a backup beside the package, and install only `Mods/PalworldGuider`.
- [ ] Prompt for provider kind and model only; require API credentials to remain process or environment scoped.
- [ ] Launch `guide-server.exe`, generate the gateway token in memory, and print `http://127.0.0.1:8070/`.
- [ ] Preserve unrelated mods and UE4SS during uninstall.
- [ ] Document download, hash verification, install, provider setup, start, game test, logs, uninstall, and blocked-build behavior.

### Level 2 Acceptance Gate

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy -p guide-server --all-targets -- -D warnings`
- [ ] `cargo test -p guide-server --test adapter_scripts`
- [ ] Release ZIP hash verification fails when any packaged file is changed.
- [ ] Installer refuses unsupported build `25094871` unless Task 3 explicitly cleared it.
- [ ] Installer requires and verifies a save backup.
- [ ] Installer preserves unrelated mods.
- [ ] A clean Windows machine without Rust or VS Build Tools can install, start, and receive `!guide ping`.
- [ ] Web guide remains available while adapter mode is active.
- [ ] Commit and push `feat: add mod installer release package` to both remotes.
