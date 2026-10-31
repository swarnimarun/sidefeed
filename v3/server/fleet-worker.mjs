import {Store} from './store.mjs';
import {consumeStream} from './fleet-client.mjs';
import {setTimeout as delay} from 'node:timers/promises';
const url=new URL(process.env.SIDEFEED_COORDINATOR||'');
if(url.protocol!=='https:'&&!(url.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(url.hostname)))throw new Error('Coordinator requires HTTPS, except local testing');
if(url.username||url.password)throw new Error('URL credentials prohibited');
const token=process.env.SIDEFEED_PEER_TOKEN;if(!token)throw new Error('SIDEFEED_PEER_TOKEN required');
const sourceMap=JSON.parse(process.env.SIDEFEED_SOURCE_MAP||'{}');if(!Object.keys(sourceMap).length)throw new Error('SIDEFEED_SOURCE_MAP must map coordinator source IDs to existing local source IDs');
const store=new Store(process.env.SIDEFEED_DB||'data/sidefeed.sqlite');for(const local of Object.values(sourceMap))if(!store.get('sources',local))throw new Error(`Missing local source ${local}`);
const controller=new AbortController();for(const sig of ['SIGINT','SIGTERM'])process.on(sig,()=>controller.abort());
let backoff=1000;
try{while(!controller.signal.aborted){try{await consumeStream({url:url.href,token,store,sourceMap,signal:controller.signal,onEvent:()=>{backoff=1000;}});}catch(e){if(controller.signal.aborted)break;if(/HTTP (401|403)/.test(e.message))throw e;console.error('Fleet stream interrupted; retrying:',e.message);}await delay(backoff,undefined,{signal:controller.signal}).catch(()=>{});backoff=Math.min(backoff*2,30000);}}finally{store.close();}
