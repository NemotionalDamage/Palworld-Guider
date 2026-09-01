pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Palworld Guider</title>
  <style>
    body { margin: 0; font: 15px/1.45 system-ui, sans-serif; color: #1f2933; background: #f5f7fa; }
    main { max-width: 880px; margin: 0 auto; padding: 24px; }
    section { background: white; border: 1px solid #d9e2ec; border-radius: 10px; padding: 16px; margin-bottom: 16px; }
    textarea { width: 100%; box-sizing: border-box; font: inherit; min-height: 110px; margin-top: 6px; }
    button { margin-top: 10px; padding: 8px 14px; }
    .muted { color: #52606d; font-size: 13px; }
    .error { color: #a4262c; white-space: pre-wrap; }
    .metadata { white-space: pre-wrap; }
    article { border-top: 1px solid #d9e2ec; padding-top: 10px; margin-top: 10px; }
  </style>
</head>
<body>
<main>
  <h1>Palworld Guider</h1>
  <section>
    <h2>Session</h2>
    <button id="create-session">Start local session</button>
    <p class="muted" id="session-status">No active session</p>
  </section>
  <section>
    <h2>Optional user-entered snapshot</h2>
    <p class="muted">Paste explicit JSON. It stays in memory and is summarized before model use.</p>
    <textarea id="snapshot" spellcheck="false"></textarea>
    <button id="import-snapshot">Import snapshot</button>
    <p class="metadata" id="snapshot-metadata"></p>
  </section>
  <section>
    <h2>Question</h2>
    <label for="question">Ask the guide</label>
    <textarea id="question"></textarea>
    <button id="ask">Ask</button>
    <p class="error" id="error"></p>
    <p class="metadata" id="answer"></p>
    <p class="metadata" id="provenance"></p>
    <p class="metadata" id="uncertainty"></p>
  </section>
  <section>
    <h2>Session history</h2>
    <div id="history"></div>
  </section>
</main>
<script>
const state = { sessionId: null };
const setStatus = (text) => document.getElementById('session-status').textContent = text;
const showError = (text) => document.getElementById('error').textContent = text || '';
async function request(method, path, body) {
  const options = method === 'GET' ? {} : { method, headers: {'content-type':'application/json'}, body: JSON.stringify(body || {}) };
  const response = await fetch(path, options);
  const payload = await response.json();
  if (!response.ok) throw new Error(payload.error?.message || 'Request failed');
  return payload;
}
async function createSession() {
  try { const result = await request('POST', '/api/sessions'); state.sessionId = result.session_id; setStatus(`Active session ${state.sessionId}`); showError(''); }
  catch (error) { showError(error.message); }
}
async function importSnapshot() {
  if (!state.sessionId) return showError('Start a session first');
  try {
    const snapshot = JSON.parse(document.getElementById('snapshot').value);
    const metadata = await request('POST', `/api/sessions/${state.sessionId}/snapshots`, snapshot);
    document.getElementById('snapshot-metadata').textContent = JSON.stringify(metadata, null, 2);
    showError('');
  } catch (error) { showError(error.message); }
}
async function ask() {
  if (!state.sessionId) return showError('Start a session first');
  try {
    const result = await request('POST', `/api/sessions/${state.sessionId}/ask`, { question: document.getElementById('question').value });
    document.getElementById('answer').textContent = result.answer.answer || '';
    document.getElementById('provenance').textContent = JSON.stringify(result.answer.provenance, null, 2);
    document.getElementById('uncertainty').textContent = result.answer.uncertainty.join('\\n');
    showError(result.answer.errors.join('\\n'));
  } catch (error) { showError(error.message); }
}
async function loadHistory() {
  if (!state.sessionId) return;
  try {
    const session = await request('GET', `/api/sessions/${state.sessionId}`);
    document.getElementById('history').innerHTML = '';
    for (const exchange of session.exchanges) {
      const article = document.createElement('article');
      const question = document.createElement('strong'); question.textContent = exchange.question;
      const answer = document.createElement('p'); answer.textContent = exchange.answer.answer || '';
      article.append(question, answer); document.getElementById('history').append(article);
    }
  } catch (_) {}
}
document.getElementById('create-session').addEventListener('click', createSession);
document.getElementById('import-snapshot').addEventListener('click', importSnapshot);
document.getElementById('ask').addEventListener('click', async () => { await ask(); await loadHistory(); });
</script>
</body>
</html>
"#;
