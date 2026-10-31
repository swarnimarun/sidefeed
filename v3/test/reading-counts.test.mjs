import test from 'node:test';
import assert from 'node:assert/strict';
import {createApp} from '../server/app.mjs';
test('reading counts describe all stored items and filter read/source correctly',async()=>{
 const app=createApp();
 for(let n=0;n<1005;n++)app.store.put('items',{id:`item-${n}`,sourceId:n<3?'source-a':'source-b',title:'Stored story',content:'Fixture',author:n===0?'Unique fixture author':'Other author',read:n%2===0,saved:n<2,publishedAt:'2026-10-01T00:00:00Z'});
 await new Promise(resolve=>app.server.listen(0,'127.0.0.1',resolve));const base=`http://127.0.0.1:${app.server.address().port}/api/v3`;
 try {
  const {readingCounts}=await(await fetch(`${base}/status`)).json();
  assert.deepEqual(readingCounts,{all:1005,saved:2,read:503,unread:502,bySource:{'source-a':3,'source-b':1002}});
  assert.equal((await(await fetch(`${base}/items`)).json()).items.length,1000);
  const {items}=await(await fetch(`${base}/items?sourceId=source-a&read=true`)).json();assert.equal(items.length,2);assert.ok(items.every(x=>x.read&&x.sourceId==='source-a'));
  const authorMatches=await(await fetch(`${base}/items?q=Unique%20fixture%20author`)).json();assert.equal(authorMatches.items.length,1);assert.equal(authorMatches.items[0].id,'item-0');
 } finally {await app.close();}
});
