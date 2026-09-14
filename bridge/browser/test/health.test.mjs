import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { startBridge } from '../server.mjs';
test('health is authenticated and never reads a page', async t => {
  const dir = await mkdtemp(join(tmpdir(), 'bridge-health-'));
  let reads = 0;
  const bridge = await startBridge({ workspace: dir, configDir: join(dir, 'config'), adapter: { run: async () => { reads++; }, close: async () => {} } });
  t.after(async () => { await bridge.close(); await rm(dir, { recursive:true, force:true }); });
  const url = `http://127.0.0.1:${bridge.config.port}/health`;
  assert.equal((await fetch(url)).status,403);
  const headers = { authorization:`Bearer ${bridge.config.token}` };
  assert.equal((await fetch(url,{headers:{...headers,origin:'https://other.example'}})).status,403);
  const response=await fetch(url,{headers}); assert.equal(response.status,200);
  assert.deepEqual(await response.json(), { ok:true, service:'coding-tools-browser', version:1, busy:false });
  assert.equal(reads,0);
});
