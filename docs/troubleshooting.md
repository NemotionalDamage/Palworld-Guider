# Troubleshooting

Common issues, their causes, and fixes. All commands assume you run from the repository root unless noted.

## 1. Data Directory Not Found

**Symptom:** `guide-core` or `guide-server` reports it cannot load `data/reviewed`, or returns an empty/unknown result for every lookup.

**Cause:** The data directory path does not resolve relative to the current working directory. This happens when the binary is launched from outside the repository root.

**Fix:**

- Run the command from the repository root, or
- Point at the data directory explicitly with `--data`:

```powershell
cargo run -p guide-core -- --data C:\path\to\Palworld-Guider\data\reviewed lookup item Wood
cargo run -p guide-server -- --data C:\path\to\Palworld-Guider\data\reviewed --port 8070
```

## 2. Provider Connection Refused

**Symptom:** `guide-server` or `guide-agent` returns a provider error such as `connection refused` or `http error` before any answer is produced.

**Cause:** The configured LLM provider endpoint is unreachable. The server has no provider configured, the base URL is wrong, the API key is missing, or a local Ollama instance is not running.

**Fix:**

- Confirm `GUIDE_PROVIDER` is set to `ollama` or `openai`.
- For the `openai` provider, confirm `OPENAI_API_KEY` is set in the environment.
- For a custom OpenAI-compatible endpoint, confirm `GUIDE_BASE_URL` points to the chat-completions endpoint (the server normalizes a bare host by appending `/chat/completions`, but a complete URL is used as-is).
- For the `ollama` provider, confirm `OLLAMA_BASE_URL` (default `http://localhost:11434`) is reachable.
- See item 4 if Ollama is the target.

## 3. Provider Timeout

**Symptom:** The agent returns a timeout error before the provider finishes responding.

**Cause:** The provider took longer than the configured deadline. Slow models, high latency, or large tool-round budgets can trigger this.

**Fix:**

- Increase the deadline with `--timeout-seconds` (default 60):

```powershell
cargo run -p guide-server -- --data data/reviewed --port 8070 --timeout-seconds 120
```

- Use a faster or smaller model through `GUIDE_MODEL`.
- Check for network congestion or a provider under load.

## 4. Ollama Not Running

**Symptom:** Connection refused on `localhost:11434` when `GUIDE_PROVIDER=ollama`.

**Cause:** The Ollama daemon is not started on this machine.

**Fix:**

- Start Ollama:

```powershell
ollama serve
```

- Verify the model named in `GUIDE_MODEL` is pulled (`ollama list`). Pull it if missing: `ollama pull llama3.2`.

## 5. Port Already In Use

**Symptom:** `guide-server` fails to start with an address-in-use error for `127.0.0.1:8070` (or the adapter port).

**Cause:** Another process is already bound to that loopback port.

**Fix:**

- Choose a different port with `--port` (and `--adapter-port` for adapter mode):

```powershell
cargo run -p guide-server -- --data data/reviewed --port 8071
```

- Or stop the process holding the port before restarting.

## 6. Session Expired

**Symptom:** The API returns `410 Gone` for a session request.

**Cause:** The session expired or was evicted. Sessions are in-memory, bounded in count, and have a fixed lifetime.

**Fix:**

- Create a new session:

```powershell
Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:8070/api/sessions"
```

- Reattach any needed snapshot to the new session before asking again.

## 7. Rate-Limited Asks

**Symptom:** The API returns `429 Too Many Requests`.

**Cause:** The session exceeded the rolling ask rate window.

**Fix:**

- Wait approximately 60 seconds for the rolling window to clear, then retry.
- Reduce request frequency. The rate window is bounded per session and cannot be raised above the compiled limit.

## 8. Grounding Fallback Answers

**Symptom:** The reply looks like a terse evidence phrase (for example, `Need 15 Wood`) instead of a natural sentence.

**Cause:** The model's draft failed grounding validation. Strict rendering rejects drafts that contain numeric literals, English or Chinese number words, unknown slot IDs, or entities not supported by tool evidence. When a draft is invalid, the deterministic fallback renders a safe, evidence-based answer instead.

**Fix:**

- This is by design. The fallback prevents hallucinated quantities and breeding claims from reaching the player. No action is required.
- If fallbacks are frequent, check that the provider model supports the structured `submit_answer` protocol and that `GUIDE_DISABLE_REASONING` is not set when using a reasoning-capable model that needs thinking enabled.

## 9. Version Mismatch Warning

**Symptom:** Answers include an uncertainty note such as `knowledge version 1.0.3 does not match configured game version X`.

**Cause:** The configured `--game-version` differs from the knowledge base's recorded `applicable_game_version`.

**Fix:**

- Set `--game-version` to match the knowledge base (currently `1.0.3`):

```powershell
cargo run -p guide-server -- --data data/reviewed --port 8070 --game-version 1.0.3
```

- Or update the knowledge base to the new game version through the data-update workflow (see `data-updates.md`).

## 10. Adapter Connection Failures

**Symptom:** The in-game adapter cannot connect to the gateway, or `!guide` questions receive no reply while the Web interface still works.

**Cause:** The gateway token is missing or mismatched, the guide-server is not running with `--adapter-port`, the game was launched by a Steam client that did not inherit the `Start-InGameGuide.ps1` session token, or the game-side adapter is not installed or running.

**Fix:**

- Exit Palworld and re-run `Start-InGameGuide.ps1` with the game closed: it binds the token to its own PowerShell session, closes a stale Steam client, and relaunches Palworld through Steam so the game inherits the token. Do not launch Palworld from a Steam window that predates Start.
- Confirm the guide-server was started with `--adapter-port` and prints `adapter=127.0.0.1:{port}`.
- Confirm the adapter package is installed and the game is running.
- See [deployment.md](deployment.md) for build, install, and hash-verification steps.

## 11. Crash Recovery

**Symptom:** The guide-server process exited unexpectedly, or the game crashed.

**Fix:**

- If the guide-server crashed: restart it. Sessions are in-memory and will be lost; create a new session. The knowledge base on disk is never mutated by the server, so no data is lost.
- If the game crashed: restart the game, then restart the guide-server. The adapter reconnects automatically on the next `!guide` event.
- During a guide-server outage, the game keeps running; `!guide` questions receive no reply and are not retried. Once the server returns, the adapter resumes normal delivery.

## 12. Game Crashes When Entering A Save (UE4SS Heap Corruption)

**Symptom:** Palworld exits with heap corruption (`0xc0000374`, faulting module `ntdll.dll`) when loading a save while UE4SS 3.0.1 is enabled on Steam build `25094871`. The crash also reproduces with `PalworldGuider` disabled in `mods.txt` and disappears when UE4SS is disabled.

**Cause:** UE4SS 3.0.1's optional world-load hooks are unstable on this Palworld build. The Guider adapter does not use them; its chat hook is registered by name (`/Script/Pal.PalGameStateInGame:BroadcastChatMessage`).

**Fix:** In `<game>\Pal\Binaries\Win64\UE4SS-settings.ini`, keep `HookProcessInternal = 1` and `HookProcessLocalScriptFunction = 1`, and set the four world-load hooks to `0`:

```ini
HookInitGameState = 0
HookCallFunctionByNameWithArguments = 0
HookBeginPlay  = 0
HookLocalPlayerExec = 0
```

Optionally set `GuiConsoleEnabled = 0`. The save then loads normally, and `!guide ping` was verified live on build `25094871` after this change (2026-09-08). Keep the change while playing this build; only re-enable the hooks if another UE4SS mod needs them. The same fix is required before the manual-start flow in `README.md` or `docs/deployment.md` can run on `25094871`.
