# Runtime Environment Record

Live read-only game integration is deferred to Phase G5. Before that validation, record:

- Palworld build/version
- platform and installation provenance
- Windows version
- load mode
- game installation path
- adapter runtime and version
- whether the target is single-player or private multiplayer
- save backup location and procedure
- probe date and timezone

No live environment has been recorded yet.

## Phase G3 Provider Configuration

- `OPENAI_API_KEY` is required for the `openai` provider and is read from the environment only.
- `OLLAMA_BASE_URL` optionally overrides the default `http://localhost:11434` endpoint for the `ollama` provider.
- Provider secrets are never written to logs, answer envelopes, or Git.
- Provider requests honor `--timeout-seconds` (default 60) and the agent tool budget `--max-tool-calls` (default 6).
- No live OpenAI-compatible or Ollama endpoint has been recorded or validated yet; Phase G3 tests use the offline scripted mock.
