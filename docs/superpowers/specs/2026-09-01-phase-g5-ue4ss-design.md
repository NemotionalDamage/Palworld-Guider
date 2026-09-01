# Phase G5 UE4SS Read-Only In-Game Chat Design

## Decision

G5 completes through the validated local-client UE4SS path, not the private-server REST path. The decision is based on the sibling `Pal` implementation at commit `16d1c12b91ad536b1b2f9e8700eefd56590aed47`: the same Palworld Steam build `24575825` and UE4SS `3.0.1 Beta #0` already validated authenticated loopback WebSocket transport, chat input, player-visible chat output, player position, active-Otomo identity and position, reconnection, and one-active-session replacement. No sibling REST target was recorded, and REST cannot provide the required in-game chat input/output loop.

The Guider implementation remains independent from the older control-oriented project. It reuses transport structure and target-build evidence only after renaming, review, and project-specific tests. No `body-control`, movement, combat, gathering, construction, inventory mutation, or world-mutation implementation is copied.

## Target Boundary

The candidate target is the local Steam Windows client:

- Palworld App `1623730`, build `24575825`
- UE4SS `3.0.1 Beta #0`, Git SHA `d935b5b`
- Transport bound only to `127.0.0.1`
- One configured local player and one active adapter session

The target is not authorized for live validation until the project owner explicitly records the session load mode, ownership and consent scope, exact save-directory backup path and retention choice, and the final read-field allowlist. No adapter is installed and Palworld is not launched until those values are recorded and the save backup succeeds.

Private-server REST remains disabled for this target. If REST is requested later, it requires a separately recorded owned or explicitly authorized endpoint, read-only `GET` allowlist, consent scope, redaction rules, timeout, and failure tests before any request is made.

## Adapter Architecture

```text
Palworld/UE4SS Lua adapter
  -> native WinHTTP transport shim
  -> Rust loopback WebSocket gateway
  -> Guider adapter bridge
  -> GuideAgent and existing ToolRegistry
```

The native C++ shim is transport-only. It registers only bounded Lua callbacks for configure, connect, send, poll, status, and close. It contains no Palworld API, Unreal object lookup, hook, provider, tool-selection, permission, or advisory logic.

The Lua adapter owns:

- one chat-input hook
- one player-visible system-chat output call
- only explicitly approved verified read handlers
- schema-2 JSON frames, sequence validation, capability manifest, heartbeats, and bounded reconnect

The command prefix is `!guide `. Empty and whitespace-only instructions are ignored. Chat text and provider prompts are never written to logs; logs may retain only bounded character counts and safe status labels.

## Rust Gateway

A new Guider `game-gateway` crate adapts the sibling schema-2 contract to this workspace. It supports:

- loopback-only bind
- exact Bearer-token authentication from an environment variable
- strict monotonically increasing sequences per direction
- UTF-8 text frames only
- 65,536-byte frame limit and bounded queues
- hello and capability-manifest validation
- ordered `chat_message` events
- one in-flight tool call with an absolute deadline
- disconnect, reconnect, replacement-session, stale-result, timeout, and cancellation handling

A replacement authenticated connection becomes the sole active session, clears previous capabilities, and cancels pending calls. The gateway exposes adapter capabilities only after intersecting the manifest with a compile-time Guider allowlist. A manifest alone cannot make a tool model-visible.

## Read-Field Allowlist

The proposed fields are exactly the reads already verified for this target in the sibling evidence:

1. player position through valid `PlayerController.Pawn` and finite `K2_GetActorLocation()` values
2. active-Otomo character identity and finite actor position through the sibling's guarded chain

Both are read-only. The active-Otomo handler reports unavailable when no Otomo is spawned; it does not fabricate a value. Health, stamina, inventory, party enumeration, nearby actors, resource classification, and all other client fields remain unavailable until separately verified and approved for this exact build.

The project owner may reduce the allowlist to chat input/output plus player position, or chat input/output plus both verified reads. The owner may not expand it through configuration alone.

## Agent Bridge

The Guider adapter bridge consumes one chat event at a time and reuses the existing grounded Rust agent:

- existing knowledge engine, lexical retrieval, deterministic calculators, planner tools, and tool registry
- existing bounded provider loop, grounding gate, provenance, version, uncertainty, and errors
- a dedicated bounded in-game context with concise reply truncation
- adapter tools published only when the manifest and local policy allow them

Every answer is model-visible only through typed tools. Tool results retain standard envelopes. Final in-game replies state missing or stale knowledge clearly. Provenance and uncertainty remain inspectable through the Web interface; the in-game reply remains concise.

## Safety And Errors

The adapter fails closed with typed errors for missing game objects, unavailable reads, invalid frames, authentication failure, timeout, disconnect, provider failure, malformed chat, and cancelled calls. It does not fabricate game state or fall back to invented facts.

No arbitrary file path, shell command, SQL, raw save access, IP address, platform user ID, credential, full actor list, or unrelated private state is exposed to the model. No mutation capability is registered. Chat output is the only UI-visible side effect.

## Verification

Offline gates precede any installation:

- Rust gateway protocol and WebSocket tests
- replacement, stale-response, timeout, cancellation, and authentication tests
- adapter bridge tests proving chat event to grounded answer to chat reply
- manifest intersection and adversarial-capability tests
- Lua source-contract tests for prefix handling, finite values, allowlisted handlers, and absence of control calls
- native shim export and dependency inspection after build
- full workspace format, lint, tests, and diff checks

Live gates then require, in order:

1. Palworld closed
2. explicit target values recorded
3. selected save directory copied to the approved backup location and verified
4. package hashes recorded
5. adapter installed under `Mods/PalworldGuider`
6. guide server and gateway started
7. Palworld launched in the recorded mode
8. player sends `!guide ping`
9. player sends one factual question and receives a concise grounded reply
10. player sends one state question and receives either verified state or a clear unavailable envelope
11. player confirms each reply was visible in game
12. Palworld exits without adapter-induced crash

A native call succeeding is not accepted as chat-output evidence; the player must visibly receive the reply. A successful run does not promote any field beyond the approved allowlist.
