import { createHash } from 'node:crypto';

export const METHODS = ['browser_list_tabs', 'browser_read_page', 'browser_screenshot', 'browser_open'];
export const MAX_IMAGE_BYTES = 2 * 1024 * 1024;
export class BridgeError extends Error {
  constructor(code, message) { super(message); this.code = code; }
}
export function normalizeWorkspace(path) {
  return path.replace(/^\\\\\?\\UNC\\/, '//').replace(/^\\\\\?\\/, '').replaceAll('\\', '/');
}
export function workspaceKey(path) {
  return createHash('sha256').update(normalizeWorkspace(path)).digest('hex');
}
export function validateRequest(method, args) {
  if (!METHODS.includes(method)) throw new BridgeError('INVALID_ARGUMENT', 'Unknown browser tool');
  if (!args || typeof args !== 'object' || Array.isArray(args)) throw new BridgeError('INVALID_ARGUMENT', 'arguments must be an object');
  const allowed = method === 'browser_open' ? ['url'] : method === 'browser_list_tabs' ? ['cursor', 'limit'] : method === 'browser_read_page' ? ['tab_id', 'cursor', 'max_chars'] : ['tab_id'];
  for (const key of Object.keys(args)) if (!allowed.includes(key)) throw new BridgeError('INVALID_ARGUMENT', `Unknown argument: ${key}`);
  if ('tab_id' in args && (typeof args.tab_id !== 'string' || !/^[a-zA-Z0-9_-]{1,80}$/.test(args.tab_id))) throw new BridgeError('INVALID_ARGUMENT', 'Invalid tab_id');
  for (const [key, min, max] of [['cursor', 0, 10000000], ['limit', 1, 100], ['max_chars', 1, 32000]]) {
    if (key in args && (!Number.isSafeInteger(args[key]) || args[key] < min || args[key] > max)) throw new BridgeError('INVALID_ARGUMENT', `Invalid ${key}`);
  }
  if (method === 'browser_open') {
    const raw = args.url;
    if (typeof raw !== 'string' || Buffer.byteLength(raw, 'utf8') > 2048 || /[\s\x00-\x1f\x7f\\]/u.test(raw) || !/^https?:\/\//i.test(raw)) throw new BridgeError('INVALID_ARGUMENT', 'url must be an HTTP/HTTPS URL of at most 2048 UTF-8 bytes without whitespace or credentials');
    let url;
    try { url = new URL(raw); } catch { throw new BridgeError('INVALID_ARGUMENT', 'Invalid URL'); }
    if (!url.hostname || url.username || url.password) throw new BridgeError('INVALID_ARGUMENT', 'URL credentials are not allowed');
  }
  return args;
}

// This function runs in the Playwright server's Node context, not as caller-supplied code.
// Only the fixed page extraction below runs in the page's isolated execution context.
export async function readBrowser(page, method, args) {
  const fail = (code, message) => ({ ok: false, error: { code, message } });
  const permitted = p => !p.isClosed() && /^https?:\/\//i.test(p.url());
  const all = page.context().pages().filter(permitted);
  const id = p => {
    if (!p.__codingToolsReadOnlyId) Object.defineProperty(p, '__codingToolsReadOnlyId', { value: `tab_${Date.now().toString(36)}_${Math.random().toString(36).slice(2)}` });
    return p.__codingToolsReadOnlyId;
  };
  const meta = async p => ({ tab_id: id(p), title: (await p.title()).slice(0, 1024), url: p.url().slice(0, 8192), selected: p === page });
  if (method === 'browser_open') {
    let opened;
    try {
      opened = await page.context().newPage();
      await opened.goto(args.url, { waitUntil: 'domcontentloaded', timeout: 10000 });
      if (!permitted(opened)) throw new Error('Unsupported destination');
      return { ok: true, ...await meta(opened), selected: true, content_trust: 'untrusted_web_content' };
    } catch {
      if (opened) await opened.close().catch(() => {});
      return fail('NAVIGATION_FAILED', 'Could not open the URL. The previous default tab is unchanged. Check the URL or connection and retry.');
    }
  }
  if (method === 'browser_list_tabs') {
    const cursor = args.cursor ?? 0;
    const limit = args.limit ?? 50;
    const tabs = [];
    for (const p of all.slice(0, 200).slice(cursor, cursor + limit)) {
      try { tabs.push(await meta(p)); } catch { /* A closing tab is omitted, never replaced. */ }
    }
    return { ok: true, tabs, selected_tab_id: permitted(page) ? id(page) : null, next_cursor: cursor + limit < Math.min(all.length, 200) ? cursor + limit : null, truncated: all.length > 200 };
  }
  const target = args.tab_id ? all.find(p => id(p) === args.tab_id) : page;
  if (!target || target.isClosed()) return fail('TAB_NOT_FOUND', 'The selected tab is closed or no longer connected. Select it locally and reconnect.');
  if (!permitted(target)) return fail('UNSUPPORTED_PAGE', 'Only HTTP and HTTPS pages may be read.');
  const before = target.url();
  const metadata = await meta(target);
  if (method === 'browser_read_page') {
    const result = await target.locator('body').evaluate((body, options) => {
      const text = body.innerText;
      // Unicode code points: cursors never split an emoji or CJK character.
      let total = 0; const out = [];
      for (const char of text) { if (total >= options.cursor && out.length < options.limit) out.push(char); total++; }
      return { text: out.join(''), total_chars: total, next_cursor: options.cursor + out.length < total ? options.cursor + out.length : null };
    }, { cursor: args.cursor ?? 0, limit: args.max_chars ?? 16000 }, { timeout: 10000 });
    if (target.url() !== before) return fail('PAGE_CHANGED', 'Page navigated during reading. Refresh the tab list and retry.');
    return { ok: true, ...metadata, ...result, content_trust: 'untrusted_web_content', scope: 'main_frame_visible_text' };
  }
  const bytes = await target.screenshot({ type: 'jpeg', quality: 70, fullPage: false, scale: 'css', timeout: 10000 });
  if (target.url() !== before) return fail('PAGE_CHANGED', 'Page navigated during screenshot. Retry.');
  if (bytes.length > 2 * 1024 * 1024) return fail('OUTPUT_TOO_LARGE', 'Viewport screenshot exceeds 2 MiB. Reduce the browser window size.');
  return { ok: true, ...metadata, mime_type: 'image/jpeg', base64: bytes.toString('base64'), bytes: bytes.length, content_trust: 'untrusted_web_content' };
}

export function decodeResult(result) {
  const text = result.content?.filter(c => c.type === 'text').map(c => c.text).join('\n') ?? '';
  if (result.isError) {
    if (text.includes('Playwright Extension not found')) throw new BridgeError('BROWSER_EXTENSION_MISSING', 'Install the official Playwright Extension in Chrome, then restart this bridge. See https://github.com/microsoft/playwright/tree/main/packages/extension#readme');
    throw new BridgeError('BROWSER_UNAVAILABLE', 'Browser disconnected, tab closed, or page could not be read. Reconnect locally.');
  }
  // Pinned Playwright response: the result is a single JSON line, before code/log sections.
  const match = text.match(/^### Result\r?\n([^\n]+)/m);
  if (!match) throw new BridgeError('BRIDGE_PROTOCOL_ERROR', 'Unsupported Playwright response format');
  try {
    const value = JSON.parse(match[1]);
    if (typeof value?.ok !== 'boolean') throw new Error();
    return value;
  } catch { throw new BridgeError('BRIDGE_PROTOCOL_ERROR', 'Invalid Playwright result'); }
}
