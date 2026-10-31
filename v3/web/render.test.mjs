import test from 'node:test';
import assert from 'node:assert/strict';
import {threadPanel} from './components/thread.js';
import {modelPanel} from './components/models.js';
const escape=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const safeUrl=v=>{try{const u=new URL(v);return ['https:','http:'].includes(u.protocol)?escape(u.href):'#';}catch{return '#';}};
const helpers={escape,safeUrl,date:()=> 'Oct 31',pill:t=>escape(t),heading:()=>'<h1>Models</h1>'};
test('thread renderer preserves unavailable nodes, escapes content, and refuses script URLs',()=>{
 const html=threadPanel({nodes:[{uri:'root',state:'post',content:'<img src=x onerror=alert(1)>',author:'<script>',url:'javascript:alert(1)'},{uri:'blocked',state:'blocked'},{uri:'missing',state:'not-found'}],edges:[{from:'blocked',to:'root',type:'reply'}],partial:true,reasons:['<script>']},helpers);
 assert.match(html,/Blocked post/);assert.match(html,/Post not found/);assert.match(html,/&lt;img/);assert.match(html,/Partial snapshot/);assert.doesNotMatch(html,/<script>|<img|href="javascript:/);
});
test('thread renderer terminates on cycles and caps deep/untrusted input',()=>{
 const html=threadPanel({nodes:[{uri:'a',state:'post',content:'alpha'},{uri:'b',state:'post',content:'beta'}],edges:[{from:'a',to:'b',type:'reply'},{from:'b',to:'a',type:'reply'}]},helpers);
 assert.equal((html.match(/alpha/g)||[]).length,1);assert.equal((html.match(/beta/g)||[]).length,1);assert.ok(html.length<10000);
});
test('unconfigured model panel explicitly disables remote actions and never fabricates jobs',()=>{
 const html=modelPanel({...helpers,status:{enabled:false},items:[{id:'x',title:'<script>alert(1)</script>'}]});
 assert.match(html,/Model processing is unavailable/);assert.match(html,/fieldset class="item-picker" disabled/);assert.match(html,/No model jobs have been requested/);assert.doesNotMatch(html,/<script>/);
});
test('model evidence and similarity values render safely with provider provenance',()=>{
 const html=modelPanel({...helpers,status:{enabled:true,provider:'test-provider',models:{classification:'m',embedding:'e'}},jobs:[{id:'j',operation:'classify',status:'failed',error:'<img>'}],artifacts:[{jobId:'j',model:'m',inputHash:'abc'}],similar:{model:'e',dimensions:3,items:[{item:{title:'<svg>',content:'safe'},score:0.12345}]}});
 assert.match(html,/inputHash/);assert.match(html,/0\.123/);assert.match(html,/&lt;img&gt;/);assert.doesNotMatch(html,/<svg>|<img>/);
});

test('actual modular router preserves newer search input on duplicate events and resets genuine route changes',async()=>{const {createRouter}=await import('./router.js');const old={window:globalThis.window,location:globalThis.location,history:globalThis.history};const handlers={};let loads=0;globalThis.window={addEventListener:(name,fn)=>handlers[name]=fn};globalThis.location={hash:'#inbox'};globalThis.history={pushState:(_a,_b,url)=>{location.hash=url}};try{const s={view:'inbox',query:'old',filter:'unread',sourceFilter:'s',menuOpen:true,selected:'a'};const navigate=createRouter(s,()=>loads++);navigate('search');s.query='newer input';handlers.hashchange();assert.equal(s.query,'newer input');assert.equal(loads,1);assert.equal(s.menuOpen,false);assert.equal(s.sourceFilter,'');location.hash='#saved';handlers.popstate();assert.equal(s.view,'saved');assert.equal(s.query,'');assert.equal(loads,2);}finally{for(const key of Object.keys(old)){if(old[key]===undefined)delete globalThis[key];else globalThis[key]=old[key]}}});

import {relatedEntries} from './reading-context.js';
import {readFileSync} from 'node:fs';
test('reading context only recommends real metadata matches and excludes unrelated/self entries',()=>{
 const lead={id:'a',sourceId:'one',tags:['Science']};
 const matches=relatedEntries(lead,[lead,{id:'b',sourceId:'one',tags:[]},{id:'c',sourceId:'two',tags:['Science']},{id:'d',sourceId:'three',tags:[]}]);
 assert.deepEqual(matches.map(m=>m.item.id),['c','b']);
 assert.equal(matches[0].reason,'Shared keyword: Science');assert.equal(matches[1].reason,'Same source');
 assert.deepEqual(relatedEntries(null,[]),[]);
});
test('web templates keep all styles in CSS under the unchanged CSP',()=>{
 for(const file of ['app.js','components/shell.js','pages/reader.js','pages/workspace.js','components/models.js','components/thread.js','index.html']){
  const source=readFileSync(new URL(file,import.meta.url),'utf8');
  assert.doesNotMatch(source,/\sstyle\s*=|<style[\s>]/i,file);
 }
 assert.match(readFileSync(new URL('pages/reader.js',import.meta.url),'utf8'),/data-item-id=/);assert.match(readFileSync(new URL('components/shell.js',import.meta.url),'utf8'),/aria-label="Archive"/);
});
test('skip link remains keyboard-focusable and clipped until focus rather than offscreen',()=>{
 const css=readFileSync(new URL('styles.css',import.meta.url),'utf8');
 const hidden=css.match(/\.skip\{([^}]+)\}/)?.[1]||'';
 const focused=css.match(/\.skip:focus\{([^}]+)\}/)?.[1]||'';
 assert.match(hidden,/clip-path:inset\(50%\)/);assert.match(hidden,/width:1px/);assert.match(hidden,/height:1px/);assert.match(hidden,/overflow:hidden/);
 assert.doesNotMatch(hidden,/display:none|visibility:hidden|top:-/);
 assert.match(focused,/clip-path:none/);assert.match(focused,/width:auto/);assert.match(focused,/height:auto/);
 const index=readFileSync(new URL('index.html',import.meta.url),'utf8');assert.match(index,/<a class="skip" href="#main">Skip to content<\/a>/);
 const shell=readFileSync(new URL('components\/shell.js',import.meta.url),'utf8');assert.match(shell,/<main id="main" tabindex="-1"/);
});

test('skip action bypasses routing and focuses main without changing the active page',()=>{
 const app=readFileSync(new URL('app.js',import.meta.url),'utf8');
 assert.match(app,/a\[href\^="#"\]:not\(\.skip\)/);
 const action=app.match(/function skipToContent\(event\)\{([^}]+)\);\}/)?.[1]||app.split('function skipToContent(event){')[1]?.split('function bind(){')[0]||'';
 assert.match(action,/event\.preventDefault\(\)/);assert.match(action,/main\?\.focus\(\)/);assert.doesNotMatch(action,/navigate|location\.hash|pushState/);
});
