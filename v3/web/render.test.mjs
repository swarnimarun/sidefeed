import test from 'node:test';
import assert from 'node:assert/strict';
import {threadPanel} from './thread-ui.js';
import {modelPanel} from './model-ui.js';
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

import {transitionView} from './navigation.js';
test('synchronous search navigation and a late duplicate hash event preserve newer input',()=>{
 const state={view:'inbox',query:'',selected:'old',filter:'unread'};
 assert.equal(transitionView(state,'search'),true);
 state.query='not-found-xyz'; // User input after synchronous click navigation.
 assert.equal(transitionView(state,'search'),false); // queued hashchange, not a new navigation
 assert.equal(state.query,'not-found-xyz');
 assert.equal(state.selected,null);
 assert.equal(transitionView(state,'saved'),true); // Real back/forward route changes still apply.
 assert.equal(state.query,'');
});

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
 for(const file of ['app.js','model-ui.js','thread-ui.js','index.html']){
  const source=readFileSync(new URL(file,import.meta.url),'utf8');
  assert.doesNotMatch(source,/\sstyle\s*=|<style[\s>]/i,file);
 }
 const source=readFileSync(new URL('app.js',import.meta.url),'utf8');
 assert.match(source,/data-item-id=/);assert.match(source,/aria-label="Archive"/);
});
