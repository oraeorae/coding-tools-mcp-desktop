import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, stat, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium } from 'playwright';
import http from 'node:http';
import { createAdapter } from '../adapter.mjs';
import { startBridge } from '../server.mjs';
import { validateRequest, decodeResult, workspaceKey, normalizeWorkspace } from '../core.mjs';

test('strict read-only request contract', () => {
  for (const method of ['browser_list_tabs', 'browser_read_page', 'browser_screenshot']) assert.doesNotThrow(() => validateRequest(method, {}));
  for (const args of [{ code:'process.exit()' }, { url:'https://example.com' }, { tab_id:"x');evil" }, { cursor:-1 }, { cursor:0.1 }, { max_chars:32001 }, null, []]) assert.throws(() => validateRequest('browser_read_page', args));
  assert.throws(() => validateRequest('browser_click', {}));
  assert.throws(() => validateRequest('browser_screenshot', { path:'/tmp/leak' }));
  assert.throws(() => validateRequest('browser_list_tabs', { limit:101 }));
  assert.throws(() => decodeResult({isError:true,content:[{type:'text',text:'Playwright Extension not found'}]}), {code:'BROWSER_EXTENSION_MISSING'});
  assert.throws(() => decodeResult({ content:[{type:'text', text:'wrong format'}] }));
  assert.equal(normalizeWorkspace('\\\\?\\C:\\code'), 'C:/code');
  assert.equal(workspaceKey('C:\\code'), workspaceKey('C:/code'));
});

test('loopback auth, origin/host denial, bounded calls and cleanup', async t => {
  const dir = await mkdtemp(join(tmpdir(), 'browser-bridge-test-'));
  let calls = 0;
  const bridge = await startBridge({workspace:dir, configDir:join(dir,'config'), adapter:{run:async () => { calls++; return {ok:true,tabs:[]}; }, close:async () => {}}});
  t.after(async () => { await bridge.close(); await rm(dir,{recursive:true,force:true}); });
  const url = `http://127.0.0.1:${bridge.config.port}/call`;
  const headers = {authorization:`Bearer ${bridge.config.token}`, 'Content-Type':'application/json'};
  const body = JSON.stringify({method:'browser_list_tabs', arguments:{}});
  for (const extra of [{authorization:'bad'}, {Origin:'https://evil.example'}]) {
    assert.equal((await fetch(url,{method:'POST',headers:{...headers,...extra},body})).status,403);
  }
  // fetch may normalize Host, so use raw HTTP to exercise DNS-rebinding rejection.
  const wrongHost = await new Promise((resolve,reject) => {
    const req = http.request(url,{method:'POST',headers:{...headers,Host:`evil.example:${bridge.config.port}`}},res=>{res.resume();resolve(res.statusCode);});
    req.on('error',reject);req.end(body);
  });
  assert.equal(wrongHost,403);
  assert.equal(calls,0);
  assert.equal((await fetch(url,{method:'POST',headers,body})).status,200);
  assert.equal(calls,1);
  assert.equal((await fetch(url,{method:'POST',headers,body:JSON.stringify({method:'browser_read_page',arguments:{code:'evil'}})})).status,400);
  assert.equal((await fetch(url,{method:'POST',headers,body:'x'.repeat(5000)})).status,400);
  assert.equal(calls,1);
  const config = JSON.parse(await readFile(bridge.configPath,'utf8'));
  assert.equal(config.workspace,(await realpath(dir)).replaceAll('\\','/'));
  if (process.platform !== 'win32') assert.equal((await stat(bridge.configPath)).mode & 0o777,0o600);
  await assert.rejects(startBridge({workspace:dir,configDir:join(dir,'config'),adapter:{}}),/already exists/);
});

test('timeout fails closed; concurrent reads cannot change selection', async t => {
  const dir = await mkdtemp(join(tmpdir(),'browser-timeout-'));
  let closed = false;
  const bridge = await startBridge({workspace:dir,configDir:join(dir,'config'),timeoutMs:70,adapter:{run:()=>new Promise(()=>{}),close:async()=>{closed=true;}}});
  t.after(async()=>{await bridge.close(); await rm(dir,{recursive:true,force:true});});
  const request = () => fetch(`http://127.0.0.1:${bridge.config.port}/call`,{method:'POST',headers:{authorization:`Bearer ${bridge.config.token}`},body:JSON.stringify({method:'browser_list_tabs',arguments:{}})});
  const first=request();
  await new Promise(resolve=>setTimeout(resolve,20));
  assert.equal((await request()).status,429);
  assert.equal((await (await first).json()).error.code,'BROWSER_TIMEOUT');
  assert.equal(closed,true);
  assert.equal((await request()).status,503);
});

test('real Chrome via Playwright: selected tab, text, screenshot, close and stale IDs', {timeout:60000}, async t => {
  const dir=await mkdtemp(join(tmpdir(),'browser-real-'));
  const web=http.createServer((req,res)=>{if(req.url==='/fail'){req.socket.destroy();return;} if(req.url==='/redirect'){res.writeHead(302,{Location:'/opened'});res.end();return;} res.setHeader('Content-Type','text/html; charset=utf-8');res.end('<html><head><title>Browser fixture</title></head><body><h1>浏览器验收😀</h1><p>'+req.url+'</p><div hidden>HIDDEN_SECRET</div><input type="password" value="PASSWORD_SECRET"></body></html>');});
  await new Promise(resolve=>web.listen(0,'127.0.0.1',resolve));
  const browser=await chromium.launch({headless:true, ...(process.env.BROWSER_TEST_EXECUTABLE ? {executablePath:process.env.BROWSER_TEST_EXECUTABLE}:{})});
  const context=await browser.newContext({viewport:{width:800,height:600}});
  const selected=await context.newPage(); await selected.goto(`http://127.0.0.1:${web.address().port}/selected`);
  const second=await context.newPage(); await second.goto(`http://127.0.0.1:${web.address().port}/second`);
  const adapter=await createAdapter({contextGetter:async()=>context,outputDir:dir});
  t.after(async()=>{await adapter.close();await browser.close();await new Promise(resolve=>web.close(resolve));await rm(dir,{recursive:true,force:true});});
  const tabs=await adapter.run('browser_list_tabs',{});
  assert.equal(tabs.tabs.length,2);
  const initial=tabs.tabs.find(tab=>tab.selected);
  // createConnection(contextGetter) selects its first page, extension mode selects the user's chosen page.
  assert.match(initial.url,/selected$/);
  const read=await adapter.run('browser_read_page',{});
  assert.match(read.text,/浏览器验收😀/); assert.match(read.text,/selected/);
  assert.doesNotMatch(read.text,/HIDDEN_SECRET|PASSWORD_SECRET/);
  let collected='',cursor=0;
  do { const part=await adapter.run('browser_read_page',{cursor,max_chars:3}); collected+=part.text;cursor=part.next_cursor; } while(cursor!==null);
  assert.equal(collected,read.text);
  const other=tabs.tabs.find(tab=>!tab.selected);
  assert.match((await adapter.run('browser_read_page',{tab_id:other.tab_id})).text,/second/);
  assert.match((await adapter.run('browser_read_page',{})).text,/selected/);
  const shot=await adapter.run('browser_screenshot',{});
  assert.equal(shot.mime_type,'image/jpeg');assert.ok(Buffer.from(shot.base64,'base64').subarray(0,3).equals(Buffer.from([255,216,255])));
  assert.equal((await adapter.run('browser_read_page',{tab_id:'missing'})).error.code,'TAB_NOT_FOUND');
  await selected.close();
  const closed=await adapter.run('browser_read_page',{});
  assert.equal(closed.ok,false);assert.equal(closed.error.code,'TAB_NOT_FOUND');
  const base = `http://127.0.0.1:${web.address().port}`;
  const opened = await adapter.run('browser_open', {url:base+'/redirect'});
  assert.equal(opened.ok,true,JSON.stringify(opened));
  assert.match(opened.url,/opened$/); assert.equal(opened.selected,true);
  assert.equal(second.url(),base+'/second');
  assert.equal(context.pages().length,2);
  assert.equal((await adapter.run('browser_list_tabs',{})).selected_tab_id,opened.tab_id);
  const newRead = await adapter.run('browser_read_page',{});
  assert.equal(newRead.tab_id,opened.tab_id); assert.equal(newRead.selected,true);assert.match(newRead.text,/opened/);
  assert.equal((await adapter.run('browser_screenshot',{})).tab_id,opened.tab_id);
  assert.equal((await adapter.run('browser_read_page',{tab_id:other.tab_id})).selected,false);
  const failure = await adapter.run('browser_open',{url:base+'/fail'});
  assert.equal(failure.error.code,'NAVIGATION_FAILED');
  assert.equal(context.pages().length,2);
  assert.equal((await adapter.run('browser_read_page',{})).tab_id,opened.tab_id);
  assert.equal((await adapter.run('browser_list_tabs',{})).selected_tab_id,opened.tab_id);
});

 test('browser_open requires a bounded safe URL and no executable arguments', () => {
  for (const url of ['https://example.com/article?q=中文', 'http://localhost:1234/path']) assert.doesNotThrow(() => validateRequest('browser_open', {url}));
  for (const args of [{}, {url:null}, {url:42}, {url:'javascript:alert(1)'}, {url:'file:///etc/passwd'}, {url:'chrome://settings'}, {url:'https://user:pass@example.com'}, {url:'https://example.com/ x'}, {url:'https://'}, {url:'https://example.com/'+'中'.repeat(700)}, {url:'https://example.com',tab_id:'x'}]) assert.throws(() => validateRequest('browser_open', args));
});
