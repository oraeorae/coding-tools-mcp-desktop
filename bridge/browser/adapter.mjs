import { createConnection } from '@playwright/mcp';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { BridgeError, decodeResult, readBrowser, validateRequest } from './core.mjs';

export async function createAdapter({ contextGetter, outputDir } = {}) {
  const server = await createConnection({
    extension: !contextGetter,
    snapshot: { mode: 'none' },
    timeouts: { action: 10000, navigation: 10000, settle: 0 },
    saveSession: false,
    codegen: 'none',
    outputDir,
  }, contextGetter);
  const client = new Client({ name: 'coding-tools-browser-readonly', version: '0.1.0' });
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  let closed = false;
  let selectedId;
  return {
    async run(method, args, timeout = 20000) {
      validateRequest(method, args);
      if (closed) throw new BridgeError('BROWSER_UNAVAILABLE', 'Bridge disconnected. Restart it locally.');
      if (method !== 'browser_list_tabs' && method !== 'browser_open' && !args.tab_id) {
        if (!selectedId) throw new BridgeError('TAB_NOT_FOUND', 'Select a tab locally before reading.');
        args = { ...args, tab_id: selectedId };
      }
      const code = `async (page) => (${readBrowser.toString()})(page, ${JSON.stringify(method)}, ${JSON.stringify(args)})`;
      // General Playwright tools stay entirely in memory. No arbitrary MCP forwarding.
      const result = await client.callTool({ name: 'browser_run_code_unsafe', arguments: { code } }, undefined, { timeout });
      const value = decodeResult(result);
      if (method === 'browser_open' && value.ok) selectedId = value.tab_id;
      if (value.tab_id) value.selected = value.tab_id === selectedId;
      if (method === 'browser_list_tabs' && value.ok) {
        if (selectedId === undefined) selectedId = value.selected_tab_id;
        value.selected_tab_id = selectedId ?? null;
        for (const tab of value.tabs) tab.selected = tab.tab_id === selectedId;
      }
      return value;
    },
    async close() {
      if (closed) return;
      closed = true;
      try { await client.close(); } finally { await server.close(); }
    },
  };
}
