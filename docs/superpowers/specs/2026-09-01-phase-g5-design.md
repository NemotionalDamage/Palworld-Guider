# Phase G5 Integrated Interfaces Design

## Goal

Expose the existing grounded advisor through stable interfaces, starting with a local Web experience and ending with a read-only in-game chat path for one explicitly recorded target.

## Architecture

The new `guide-server` crate owns HTTP transport, request validation, session management, response envelopes, static browser assets, and interface diagnostics. It delegates all language understanding, tool selection, deterministic calculations, planning, and game facts to the existing `GuideAgent`, `guide-tools`, `guide-planner`, and reviewed knowledge crates.

The first milestone is a loopback-only Axum service. It binds `127.0.0.1` by default, starts only when explicitly launched, and exposes:

- `GET /health` for dependency-free readiness.
- `POST /api/sessions` to create a bounded in-memory session.
- `GET /api/sessions/{session_id}` for bounded history, provenance, uncertainty, and errors.
- `POST /api/sessions/{session_id}/snapshots` to validate and attach explicit user-entered JSON.
- `POST /api/sessions/{session_id}/ask` to submit one bounded question.
- `GET /` for a minimal static browser UI.

Sessions, snapshots, provider requests, and answers are not persisted. Session identifiers are opaque and random. History is capped to the existing bounded follow-up context. Missing or expired sessions fail clearly.

## Safety And Configuration

Provider credentials remain environment-only and are never logged. Request bodies and JSON depth are size-limited. Each session has fixed request and snapshot quotas with clear `429` responses. Every ask accepts an optional cancellation token and enforces a server-side deadline. Provider, adapter, validation, and retrieval failures return typed errors without fallback fabrication.

The snapshot endpoint accepts only explicit user-entered `state_snapshot_v1` JSON. It validates the complete value through `state-snapshot` before attachment and returns only source kind, version, freshness, completeness, and missing-field metadata. Raw snapshots, consent IDs, consent objects, file paths, credentials, and capture metadata never enter provider requests or API responses.

## Interface Sequence

Optional Tauri, Discord, or QQ adapters remain out of scope until the Web path passes its tests. Runtime private-server REST and UE4SS integration are additionally blocked on a recorded target definition containing Palworld version, platform, load mode, ownership, backup location, consent scope, read-only endpoint or object list, and exact-build field verification.

Private-server retrieval may use only configured read-only `GET` endpoints. UE4SS retrieval may expose only fields verified safe and stable for the recorded client build. Both paths normalize data into `PlayerStateSnapshot`, preserve original sources unchanged, redact unrelated private data, and fail closed when unavailable.

The final interface is a thin read-only chat adapter. It receives natural-language input, invokes the same Rust core and tool registry, and returns concise grounded replies with provenance and uncertainty. It has no movement, combat, construction, inventory, world-mutation, or arbitrary-command capability.

## Verification

Test-first coverage includes endpoint routing, malformed and oversized JSON, session creation and expiry, bounded history, snapshot validation and redaction, successful and failed asks, provenance and uncertainty propagation, rate limits, timeouts, cancellation, provider errors, static UI serving, and absence of secrets in logs and responses. Live integration is validated only after offline gates pass and uses a private local read-only session for the recorded target.
