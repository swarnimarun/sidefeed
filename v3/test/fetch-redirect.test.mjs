import test from 'node:test';
import assert from 'node:assert/strict';
import dns from 'node:dns/promises';
import https from 'node:https';
import {EventEmitter} from 'node:events';
import {safeFetch} from '../server/fetch.mjs';
test('redirect private IP blocked before second hop; conditional headers dropped cross origin',async t=>{
 t.mock.method(dns,'lookup',async()=>[{address:'8.8.8.8',family:4}]);let targets=[],requests=[];let location='http://127.0.0.1/admin';
 t.mock.method(https,'get',(u,opts,cb)=>{targets.push(u.href);requests.push(opts);const req=new EventEmitter();req.destroy=e=>{req.emit('error',e);req.emit('close');};queueMicrotask(()=>{const res=new EventEmitter();res.statusCode=targets.length===1?302:304;res.headers=targets.length===1?{location}:{};cb(res);res.emit('end');req.emit('close');});return req;});
 await assert.rejects(safeFetch('https://origin.example/feed'),/Non-public/);assert.equal(targets.length,1);
 targets=[];requests=[];location='https://other.example/feed';const r=await safeFetch('https://origin.example/feed',{headers:{accept:'application/rss+xml','if-none-match':'secret-validator'},allowStatuses:[304]});assert.equal(r.status,304);assert.equal(requests[1].headers['if-none-match'],undefined);assert.equal(requests[1].headers.accept,'application/rss+xml');
});
