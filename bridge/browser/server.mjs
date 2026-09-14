import http from 'node:http';
import { timingSafeEqual, randomBytes } from 'node:crypto';
import { mkdir, open, readFile, realpath, rm, mkdtemp } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createAdapter } from './adapter.mjs';
import { BridgeError, normalizeWorkspace, validateRequest, workspaceKey } from './core.mjs';

function authorized(actual, expected) {
  if (typeof actual !== 'string') return false;
  const a = Buffer.from(actual), b = Buffer.from(`Bearer ${expected}`);
  return a.length === b.length && timingSafeEqual(a, b);
}
function send(res, status, body) {
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' });
  res.end(JSON.stringify(body));
}
export async function startBridge({ workspace, adapter, configDir = join(homedir(), '.coding-tools-mcp', 'browser'), timeoutMs = 20000 }) {
  workspace = normalizeWorkspace(await realpath(workspace));
  const token = randomBytes(32).toString('hex');
  const configPath = join(configDir, `${workspaceKey(workspace)}.json`);
  await mkdir(configDir, { recursive: true, mode: 0o700 });
  // Exclusive creation prevents silently stealing another workspace's browser connection.
  const configFile = await open(configPath, 'wx', 0o600).catch(() => { throw new BridgeError('BRIDGE_ALREADY_CONFIGURED', `Bridge configuration already exists: ${configPath}. Stop the old bridge; remove stale configuration only if it is no longer running.`); });
  let busy = false, failed = false;
  const server = http.createServer(async (req, res) => {
    if (req.headers.host !== `127.0.0.1:${server.address().port}` || req.headers.origin !== undefined || !authorized(req.headers.authorization, token)) return send(res, 403, { ok: false, error: { code: 'FORBIDDEN', message: 'Local bridge authorization failed' } });
    if (req.method === 'GET' && req.url === '/health') return send(res, 200, { ok: !failed, service: 'coding-tools-browser', version: 1, busy });
    if (req.method !== 'POST' || req.url !== '/call') return send(res, 404, { ok: false });
    if (busy) return send(res, 429, { ok: false, error: { code: 'BROWSER_BUSY', message: 'Another browser read is running. Retry shortly.' } });
    if (failed) return send(res, 503, { ok: false, error: { code: 'BROWSER_UNAVAILABLE', message: 'Restart the browser bridge locally.' } });
    busy = true;
    let timer;
    try {
      const chunks = []; let size = 0;
      for await (const chunk of req) { size += chunk.length; if (size > 4096) throw new BridgeError('INVALID_ARGUMENT', 'Request too large'); chunks.push(chunk); }
      let body;
      try { body = JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch { throw new BridgeError('INVALID_ARGUMENT', 'Invalid JSON'); }
      if (!body || Object.keys(body).some(k => !['method', 'arguments'].includes(k))) throw new BridgeError('INVALID_ARGUMENT', 'Unknown request field');
      const args = validateRequest(body.method, body.arguments);
      const result = await Promise.race([
        adapter.run(body.method, args),
        new Promise((_, reject) => { timer = setTimeout(() => { failed = true; void adapter.close().catch(() => {}); reject(new BridgeError('BROWSER_TIMEOUT', 'Browser read timed out. Restart the bridge locally.')); }, timeoutMs); }),
      ]);
      send(res, 200, result);
    } catch (error) {
      const known = error instanceof BridgeError;
      send(res, known && error.code === 'INVALID_ARGUMENT' ? 400 : 503, { ok: false, error: { code: known ? error.code : 'BROWSER_UNAVAILABLE', message: known ? error.message : 'Browser read failed. Check the local connection.' } });
    } finally { clearTimeout(timer); busy = false; }
  });
  server.requestTimeout = 5000;
  server.headersTimeout = 5000;
  server.timeout = 25000;
  try {
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
    const config = { version: 1, workspace, port: server.address().port, token };
    await configFile.writeFile(JSON.stringify(config)); await configFile.close();
    return {
      config, configPath,
      async close() {
        server.closeAllConnections();
        await new Promise(resolve => server.close(resolve));
        try { await adapter.close(); }
        finally {
          try { const current = JSON.parse(await readFile(configPath, 'utf8')); if (current.token === token) await rm(configPath); } catch { /* Already removed. */ }
        }
      },
    };
  } catch (error) { await configFile.close(); server.close(); await rm(configPath, { force: true }); throw error; }
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== '--workspace') throw new Error('Usage: npm start -- --workspace /absolute/project/path');
  const workspace = await realpath(args[1]);
  const outputDir = await mkdtemp(join(tmpdir(), 'coding-tools-browser-'));
  let adapter, bridge;
  let stopping = false;
  async function stop() {
    if (stopping) return; stopping = true;
    try { if (bridge) await bridge.close(); else if (adapter) await adapter.close(); }
    finally { await rm(outputDir, { recursive: true, force: true }); }
  }
  if (process.env.CODING_TOOLS_DESKTOP_BRIDGE === '1') {
    process.stdin.resume();
    process.stdin.once('end', () => { void stop().finally(() => process.exit(0)); });
  }
  process.once('SIGINT', () => { void stop().finally(() => process.exit(0)); });
  process.once('SIGTERM', () => { void stop().finally(() => process.exit(0)); });
  try {
    console.log('Connecting to Playwright Extension. Select the tab to share in Chrome (up to 120 seconds).');
    adapter = await createAdapter({ outputDir });
    const initial = await adapter.run('browser_list_tabs', {}, 120000);
    if (!initial.selected_tab_id) throw new Error('Select an HTTP/HTTPS tab in the extension, then restart the bridge.');
    bridge = await startBridge({ workspace, adapter });
    console.log(`Browser bridge ready for workspace: ${workspace}\nConfiguration: ${bridge.configPath}\nOnly browser_list_tabs, browser_read_page, browser_screenshot, browser_open are available. Ctrl+C to disconnect.`);
  } catch (error) { await stop(); throw error; }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
