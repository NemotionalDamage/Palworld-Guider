# Configuration

## Environment Variables

| Variable | Required by | Description |
|---|---|---|
| `GUIDE_PROVIDER` | `guide-server` | Provider kind: `openai` or `ollama`. Required. |
| `GUIDE_MODEL` | `guide-server` | Model name passed to the provider (for example `gpt-4o-mini` or `llama3.2`). Required. |
| `OPENAI_API_KEY` | `guide-server`, `guide-agent` (openai only) | API key for the OpenAI-compatible provider. Required when `GUIDE_PROVIDER=openai` or `--provider openai`. Read from the environment only; never printed or logged. |
| `GUIDE_BASE_URL` | `guide-server` (optional) | Provider endpoint URL. When omitted, the openai provider defaults to `https://api.openai.com/v1/chat/completions` and the ollama provider defaults to `http://localhost:11434`. The server normalizes incomplete URLs by appending `/chat/completions` for the openai provider. |
| `GUIDE_DISABLE_REASONING` | `guide-server`, `guide-agent` (optional) | Set to `1` or `true` to suppress reasoning tokens for reasoning-capable models. Defaults to disabled (reasoning allowed). |
| `OLLAMA_BASE_URL` | `guide-agent` (optional) | Override for the Ollama base URL. Used only by the `guide-agent` CLI when `--base-url` is not supplied. Defaults to `http://localhost:11434`. The `guide-server` does not read this variable; it uses `GUIDE_BASE_URL` instead. |
| `PALWORLD_GUIDER_GATEWAY_TOKEN` | `guide-server` (adapter mode) | Bearer token for the loopback adapter gateway. Must be 16 to 4096 characters, contain no control characters, and have no surrounding whitespace. Read from the environment variable named by `--adapter-token-env` (default `PALWORLD_GUIDER_GATEWAY_TOKEN`). Never printed or logged. |
| `PALWORLD_GUIDER_CHAT_DEBUG_LOG` | `guide-server` (adapter mode, optional) | File path for an opt-in JSONL chat debug log. Records question, reply, delivery status, errors, uncertainty, and tool names. The path should stay under the gitignored `.local/` directory. |

### Provider secrets

Provider secrets (`OPENAI_API_KEY`, `PALWORLD_GUIDER_GATEWAY_TOKEN`) are read from the environment only. They are never written to logs, answer envelopes, error messages, or Git.

---

## CLI Flags

### guide-core

```
usage: guide-core [--data DIR] [--game-version VERSION] COMMAND...
```

| Flag | Default | Description |
|---|---|---|
| `--data DIR` | `data/reviewed` | Path to the reviewed knowledge directory. |
| `--game-version VERSION` | none | Configured game version for version-match checks. |

Commands:

| Command | Arguments | Description |
|---|---|---|
| `lookup` | `item\|pal\|technology\|recipe NAME...` | Exact entity lookup by type and name. |
| `recipe` | `NAME...` | Recipe lookup by output name. |
| `materials` | `QUANTITY NAME...` | Recursive material expansion for a craft quantity. |
| `shortage` | `[--inventory NAME=QTY,...] QUANTITY NAME...` | Material shortage after subtracting inventory. |
| `craftable` | `[--inventory NAME=QTY,...] NAME...` | Maximum craftable count given inventory. |
| `breeding` | `PARENT_A PARENT_B` | Deterministic breeding result for a parent pair. |
| `chain` | `MAX_DEPTH START TARGET` | Breeding chain from a start Pal to a target Pal. |

All commands output JSON with `status`, `data`, `provenance`, `version`, `uncertainty`, and `errors`.

### guide-server

```
usage: guide-server --data <reviewed-data-directory> [--game-version <version>] [--port <1-65535>] [--timeout-seconds <seconds>] [--adapter-port <1-65535>] [--adapter-token-env <name>]
```

| Flag | Default | Description |
|---|---|---|
| `--data DIR` | required | Path to the reviewed knowledge directory. |
| `--game-version VERSION` | none | Configured game version for version-match checks. |
| `--port PORT` | `8070` | Loopback Web server port. Must be free on `127.0.0.1`. |
| `--timeout-seconds SECS` | `30` | Ask timeout in seconds. Must be between 1 and 300. |
| `--adapter-port PORT` | none | Enables in-game adapter mode. Binds the loopback WebSocket gateway on `127.0.0.1:{port}`. Must be a free loopback port. |
| `--adapter-token-env NAME` | `PALWORLD_GUIDER_GATEWAY_TOKEN` | Environment variable name that carries the adapter bearer token. |
| `--host` | rejected | Always rejected. The server is loopback-only and always binds `127.0.0.1`. |

Provider configuration is environment-only: `GUIDE_PROVIDER`, `GUIDE_MODEL`, `GUIDE_BASE_URL`, and `OPENAI_API_KEY`. No provider credentials appear on the command line.

When `--adapter-port` is supplied, startup prints `adapter=127.0.0.1:{port}` and the token environment variable name only. The token value is never printed.

### guide-maintenance

```
usage: guide-maintenance [--data DIR] [--game-version VERSION] COMMAND...
```

| Flag | Default | Description |
|---|---|---|
| `--data DIR` | `data/reviewed` | Path to the reviewed knowledge directory. |
| `--game-version VERSION` | none | Configured game version for version-match and stale-record checks. |

Commands:

| Command | Arguments | Description |
|---|---|---|
| `version-check` | none | Report version compatibility across knowledge, index, and game dimensions. Exit `0` if no warnings, `1` if warnings. |
| `audit-sources` | none | List all registered sources with fact counts. |
| `audit-conflicts` | none | List all conflicts with resolution status. |
| `audit-stale` | none | List records whose game version does not match. Requires `--game-version`. Exit `0` if none, `1` if stale records found. |
| `validate-batch` | `FILE` | Validate JSONL records without persisting. Exit `0` if valid, `1` if invalid. |

---

## Server Limits

Default session and rate-limit values defined in `ServerLimits`:

| Limit | Value | Description |
|---|---|---|
| `session_ttl` | 1800 s (30 min) | Time-to-live for an idle session. |
| `ask_timeout` | 30 s | Maximum time for a single ask. Overridden by `--timeout-seconds`. |
| `max_sessions` | 32 | Maximum concurrent in-memory sessions. |
| `max_history_exchanges` | 4 | Number of prior question/answer exchanges included in follow-up context. |
| `max_asks_per_minute` | 10 | Rate limit for asks per session. |
| `max_snapshots_per_minute` | 3 | Rate limit for snapshot imports per session. |

---

## Payload Limits

| Limit | Value | Source |
|---|---|---|
| `max_body_bytes` | 65,536 (64 KB) | HTTP request body limit enforced by `RequestBodyLimitLayer`. |
| `max_json_depth` | 32 | Maximum nesting depth for parsed JSON request bodies. |
| `gateway_max_payload_bytes` | 65,536 (64 KB) | Maximum WebSocket frame size for the adapter gateway. |
| `gateway_call_timeout` | 3 s | Maximum adapter tool-call timeout. Capped at the ask timeout. |

---

## Log Rotation

The chat debug log (`PALWORLD_GUIDER_CHAT_DEBUG_LOG`) rotates automatically:

| Setting | Value |
|---|---|
| `max_file_bytes` | 10 MB (10,485,760 bytes) |
| `max_rotated_files` | 5 |

When the active log exceeds `max_file_bytes`, it is renamed to `{path}.rotated-{timestamp_ms}` and redacted. Rotated files beyond `max_rotated_files` are deleted. Redaction strips `Bearer` tokens, API keys (`OPENAI_API_KEY=`, `sk-`), and gateway tokens from rotated content.

---

## Provider Setup

### OpenAI-compatible

```powershell
$env:GUIDE_PROVIDER = "openai"
$env:GUIDE_MODEL = "gpt-4o-mini"
$env:OPENAI_API_KEY = "your-api-key"
# Optional: override the endpoint (for Azure, LM Studio, etc.)
$env:GUIDE_BASE_URL = "https://your-endpoint.example.com/v1/chat/completions"
```

The server normalizes incomplete base URLs by appending `/chat/completions`. Complete URLs are used as-is.

### Ollama

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
# Optional: the guide-server uses GUIDE_BASE_URL for the Ollama endpoint
$env:GUIDE_BASE_URL = "http://localhost:11434"
```

For the `guide-agent` CLI, `OLLAMA_BASE_URL` overrides the Ollama endpoint instead:

```powershell
$env:OLLAMA_BASE_URL = "http://localhost:11434"
```

### Suppressing reasoning tokens

Some reasoning-capable models emit reasoning tokens that consume context. To suppress them:

```powershell
$env:GUIDE_DISABLE_REASONING = "1"
```

---

## Safety And Read-Only Boundary

- The Web server and adapter gateway bind **only** `127.0.0.1`. The `--host` flag is rejected; no public binding is possible.
- Provider credentials are read from environment variables only and are never printed, logged, or committed.
- The adapter exposes only the compile-time model-visible tool allowlist (`get_player_status`, `get_active_pal_status`). `send_chat_message` is internal and never callable by the model.
- Snapshot responses contain only schema version, source kind, game version, freshness, and missing fields. Raw snapshots, save files, and consent data never reach provider prompts.
- **No mutation path exists**: no movement, combat, gathering, construction, inventory mutation, world mutation, save writer, arbitrary shell command, or game mutation.
- Read-only capabilities fail clearly when unavailable; failures never fabricate facts.
