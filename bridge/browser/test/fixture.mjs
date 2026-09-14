// Rust end-to-end fixture. Never connects to the user's real browser profile.
import http from 'node:http';
import { chromium } from 'playwright';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createAdapter } from '../adapter.mjs';
import { startBridge } from '../server.mjs';
const dir=await mkdtemp(join(tmpdir(),'browser-e2e-'));
const web=http.createServer((req,res)=>{res.setHeader('Content-Type','text/html; charset=utf-8');res.end(`<title>Fixture</title><h1>${req.url} fixture</h1>`);});
await new Promise(resolve=>web.listen(0,'127.0.0.1',resolve));
const browser=await chromium.launch({headless:true,...(process.env.BROWSER_TEST_EXECUTABLE?{executablePath:process.env.BROWSER_TEST_EXECUTABLE}:{})});
const context=await browser.newContext();
for(const name of ['selected','other']) {const page=await context.newPage();await page.goto(`http://127.0.0.1:${web.address().port}/${name}`);}
const adapter=await createAdapter({contextGetter:async()=>context,outputDir:dir});
await adapter.run('browser_list_tabs',{});
const bridge=await startBridge({workspace:process.argv[2],adapter});
console.log('READY browser');
let stopping=false;
async function close(){if(stopping)return;stopping=true;await bridge.close();await browser.close();web.close();await rm(dir,{recursive:true,force:true});process.exit(0);}
process.on('SIGTERM',()=>void close());process.on('SIGINT',()=>void close());
// Parent test disappearance should not leave a headless browser running.
const parent=process.ppid;
setInterval(()=>{try{process.kill(parent,0);}catch{void close();}},1000).unref();
