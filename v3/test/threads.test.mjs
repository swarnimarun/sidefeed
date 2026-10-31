import test from 'node:test';
import assert from 'node:assert/strict';
import {normalizeThread,normalizeAtPost,threadUrl} from '../server/threads.mjs';
import {parseFeed} from '../server/adapters.mjs';
const uri=(did,key='a')=>`at://did:plc:${did}/app.bsky.feed.post/${key}`;
const post=(did,extra={})=>({uri:uri(did),cid:`cid-${did}`,author:{handle:`${did}.test`},record:{text:`Post ${did}`,createdAt:'2026-10-01T00:00:00Z',...extra}});
test('AppView dispatch, stable record identity and strong reply references',()=>{
 const p=post('alice',{reply:{parent:{uri:uri('bob'),cid:'cid-b'},root:{uri:uri('carol'),cid:'cid-c'}}});
 const a=parseFeed(JSON.stringify({feed:[{post:p}]}),'https://public.api.bsky.app','atproto')[0];
 const b=normalizeAtPost({...p,cid:'edited',record:{...p.record,text:'edited'}});
 assert.equal(a.externalId,b.externalId);assert.notEqual(a.cid,b.cid);assert.equal(a.reply.root.cid,'cid-c');assert.equal(a.authorDid,'did:plc:alice');
 assert.throws(()=>parseFeed('{"items":[]}','https://example.test','atproto'),/AppView/);
});
test('cross-author replies, ancestors, root refs and blocked/not-found nodes',()=>{
 const parent=post('parent'),root=post('alice'),child=post('bob',{reply:{parent:{uri:root.uri,cid:root.cid},root:{uri:parent.uri,cid:parent.cid}}});
 const graph=normalizeThread({thread:{post:root,parent:{post:parent},replies:[{post:child},{uri:uri('blocked'),blocked:true},{uri:uri('gone'),notFound:true}]}});
 assert.equal(graph.rootUri,root.uri);assert.equal(graph.nodes.length,5);
 assert.equal(graph.nodes.find(x=>x.uri===child.uri).authorDid,'did:plc:bob');
 assert(graph.edges.some(e=>e.from===child.uri&&e.to===root.uri&&e.type==='reply'));
 assert(graph.edges.some(e=>e.from===child.uri&&e.to===parent.uri&&e.type==='root'));
 assert.equal(graph.nodes.find(x=>x.uri===uri('blocked')).state,'blocked');assert.equal(graph.nodes.find(x=>x.uri===uri('gone')).state,'not-found');
 assert.equal(graph.partial,true);assert.equal(graph.truncated,false);
});
test('missing parent/root remain placeholders and placeholder hydrates',()=>{
 const p=post('alice',{reply:{parent:{uri:uri('bob'),cid:'b'},root:{uri:uri('carol'),cid:'c'}}});
 const graph=normalizeThread({thread:{post:p,parent:{post:post('bob')}}});
 assert.equal(graph.nodes.find(x=>x.uri===uri('bob')).state,'post');assert.equal(graph.nodes.find(x=>x.uri===uri('carol')).state,'missing');
});
test('bounds broad/deep/cyclic graphs and repeated records',()=>{
 const root={post:post('alice'),replies:Array.from({length:1000},(_,i)=>({post:post(`child${i}`)}))};
 const graph=normalizeThread({thread:root},{maxNodes:7});assert(graph.nodes.length<=7);assert(graph.truncated);
 const zero=normalizeThread({thread:root},{maxDepth:0});assert.equal(zero.nodes.length,1);assert(zero.truncated);
 root.replies=[root];assert.equal(normalizeThread({thread:root}).nodes.length,1);
 assert.throws(()=>normalizeThread({thread:root},{maxNodes:501}),/limit/);
});
test('fixed public endpoint, canonical DID validation and explicit partial root',()=>{
 const u=new URL(threadUrl(uri('alice'),{depth:2}));assert.equal(u.origin,'https://public.api.bsky.app');assert.equal(u.searchParams.get('depth'),'2');
 for(const bad of ['https://localhost/a','at://alice.test/app.bsky.feed.post/a',uri('alice','..'),uri('alice')+'?x=y'])assert.throws(()=>threadUrl(bad),/DID/);
 assert.throws(()=>threadUrl(uri('alice'),{depth:100}),/limit/);
 const graph=normalizeThread({thread:{uri:uri('gone'),notFound:true}});assert.equal(graph.nodes[0].state,'not-found');
 assert.throws(()=>normalizeThread({thread:{}}),/DID/);
});
