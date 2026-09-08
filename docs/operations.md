# Operations Guide

This guide covers performance budgets, log rotation, crash recovery, backup procedures, secret redaction, and monitoring for the Palworld Guider guide-server.

## Performance Budgets

### Knowledge Store Load

Loading the reviewed knowledge base from `data/reviewed/` takes approximately 1-2 seconds for 3,600+ records. This occurs once at server startup. The store is read-only after load; the on-disk data is never mutated.

### Index Build

Building the Tantivy in-memory lexical index takes approximately 1-2 seconds. This occurs once at startup, immediately after the knowledge store loads.

### Search Latency

Lexical search queries complete in under 100 milliseconds per query. Exact identifier resolution and structured lookups are faster.

### Agent Round Latency

Agent round latency depends on the provider:

| Provider | Typical latency per round |
|---|---|
| Local Ollama | 5-30 seconds |
| OpenAI-compatible | 1-5 seconds |

Each question may involve up to 6 tool calls plus one final answer synthesis. Total ask latency is bounded by the agent timeout.

### Tool Dispatch

Deterministic tool dispatch completes in under 10 milliseconds per tool call. This includes argument validation, registry dispatch, and result envelope construction.

### Budgets and Limits

| Parameter | Value | Source |
|---|---|---|
| Max tool calls per question | 6 | `AgentLimits.max_tool_calls` |
| Max reply characters | 1,200 | `AgentConfig.max_reply_characters` |
| Agent timeout | 30 seconds | `--timeout-seconds` (default 30, range 1..=300) |
| Gateway tool-call timeout cap | 3 seconds | `GATEWAY_CALL_TIMEOUT_CAP` |
| Gateway max frame size | 65,536 bytes | `GATEWAY_MAX_PAYLOAD_BYTES` |
| Max request body size | 65,536 bytes | `MAX_BODY_BYTES` |
| Max JSON depth | 32 | `MAX_JSON_DEPTH` |
| Max question length | 2,000 characters | `ask` handler |

## Log Rotation

### Chat Debug Log

The chat debug log is opt-in via the `PALWORLD_GUIDER_CHAT_DEBUG_LOG` environment variable. When enabled, it records questions, replies, delivery status, errors, uncertainty, and tool names as JSONL. The log path must be under a gitignored directory (typically `.local/`).

### Rotation Behavior

- Rotation is checked at server startup before the adapter service begins.
- The log file is rotated when it exceeds 10 MB (`max_file_bytes = 10 * 1024 * 1024`).
- The current log is renamed to `{filename}.rotated-{timestamp_ms}`.
- Up to 5 rotated files are kept; older rotated files are deleted (`max_rotated_files = 5`).
- Rotation occurs only at startup; the log grows without bound during a running session.

### Redaction During Rotation

Rotated files are redacted of sensitive patterns before being persisted:

- `Bearer ` tokens
- `OPENAI_API_KEY=` values
- `PALWORLD_GUIDER_GATEWAY_TOKEN=` values
- `GUIDE_GATEWAY_TOKEN=` values
- `sk-` prefixed keys

Each pattern is replaced with `[REDACTED]` on the matching line. The active log file is not redacted in place; only rotated copies are.

## Crash Recovery

### Guide-Server Crash

- **Recovery:** Restart the server. In-memory sessions, history, and snapshots are lost. Knowledge data on disk is never mutated.
- **Impact:** The Web interface and adapter gateway are unavailable until restart. No game state is affected.

### Game Crash

- **Recovery:** Restart Palworld, then restart guide-server. The adapter reconnects automatically when both are running.
- **Impact:** The adapter polls and reconnects on its own. Unanswered events during the outage are ignored. The guide-server stays running and the Web interface remains available.

### Provider Unavailable

- **Recovery:** None required. The agent returns an error answer with a clear message. The server stays running.
- **Impact:** Questions during the outage receive an error response in the answer envelope. The Web interface, sessions, and adapter gateway remain operational. When the provider recovers, subsequent questions succeed without restart.

### Adapter Gateway Failure

- **Recovery:** None required. The adapter service keeps polling and reconnecting. Guide-server stays running.
- **Impact:** In-game chat replies are not delivered while the gateway is down. The Web interface is unaffected. The adapter service loop never crashes the guide-server; gateway errors are caught and polling continues.

## Backup Procedure

### Knowledge Base

`data/reviewed/` is the canonical reviewed data. Back up this directory to preserve all items, Pals, recipes, technologies, aliases, and conflicts. This is the only durable data; everything else is derivable or in-memory.

### Game Save

Before adapter deployment, use the backup script:

```powershell
.\scripts\Backup-PalworldSave.ps1
```

This copies the complete save directory to `.local/backups/g5/{world-id}/` and writes a `backup-manifest.json`. The install script refuses to proceed without a verified backup. Save backups are retained for 30 days.

### Configuration

Environment variables are not persisted by the guide-server. Document them separately:

- `GUIDE_PROVIDER` and `GUIDE_MODEL` for provider selection
- `OPENAI_API_KEY` for OpenAI-compatible providers
- `GUIDE_BASE_URL` for endpoint overrides
- `GUIDE_DISABLE_REASONING` for reasoning-capable models
- `PALWORLD_GUIDER_GATEWAY_TOKEN` for adapter mode
- `PALWORLD_GUIDER_CHAT_DEBUG_LOG` for debug logging

Never store credentials in files under version control.

### Local Research Data

`.local/` is gitignored. It may contain research exports, audit reports, debug logs, and save backups. Back up separately if needed. Do not commit this directory.

## Secret Redaction

### Environment Variables

- `OPENAI_API_KEY` and `PALWORLD_GUIDER_GATEWAY_TOKEN` are read from the environment only.
- These values are never written to logs, answer envelopes, API responses, or Git.
- The adapter token is never printed; startup output shows only the environment variable name.
- Provider construction occurs outside the async runtime to avoid credential leakage through panic backtraces.

### Snapshot Metadata

The API exposes only the following snapshot fields:

- `schema_version`
- `source_kind`
- `game_version`
- `freshness`
- `missing_fields`

Consent IDs, `captured_at` timestamps, and raw inventory contents are never included in API responses.

### Provider Requests

Provider requests never include:

- Consent IDs or consent objects
- `captured_at` timestamps
- Raw inventory or party data
- Raw save file content
- Authentication credentials

The model receives only question-relevant summaries derived from the snapshot through typed Rust tools.

### Chat Debug Logs

Chat debug logs are redacted during rotation (see Log Rotation above). The active log file records tool names and question/reply text but never credentials, bearer tokens, or API keys.

## Monitoring

### Health Endpoint

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:8070/health
```

Returns `200 OK` with `{"status":"ok"}` when the server is running. Use this for liveness checks.

### Session Management

| Parameter | Value |
|---|---|
| Session TTL | 30 minutes (1,800 seconds) |
| Max concurrent sessions | 32 |
| Max history exchanges per session | 4 |
| Ask rate limit | 10 per minute per session |
| Snapshot rate limit | 3 per minute per session |

Sessions are in-memory and expire automatically. Expired sessions return a `session_expired` error. When the 32-session cap is reached, new session creation returns a `session_limit_reached` error.

### Rate Limit Responses

When a rate limit is exceeded, the API returns a `429` error with a `retry_after_secs` field indicating when the next request may be sent.

### Startup Output

On successful startup, the server prints:

```text
Palworld Guider listening on http://127.0.0.1:{port}
```

When adapter mode is enabled, it also prints:

```text
adapter=127.0.0.1:{adapter-port}
adapter-token-env={variable-name}
```

When the chat debug log is enabled, it prints:

```text
chat-debug-log={path}
```
