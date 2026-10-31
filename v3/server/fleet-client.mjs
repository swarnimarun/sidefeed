// Explicitly configured trusted-coordinator consumer. No peer discovery or autojoin.
import {hash} from './adapters.mjs';
export function applyDelta(store,event,sourceMap,coordinator){
 const sourceId=sourceMap[event.sourceId];if(!sourceId||!store.get('sources',sourceId))throw new Error('Unmapped fleet source');
 if(event.version!==1||event.type!=='item.upsert'||!event.item||typeof event.item.externalId!=='string'||typeof event.item.title!=='string'||typeof event.item.content!=='string'||event.item.content.length>100000||!Number.isSafeInteger(event.seq))throw new Error('Invalid fleet event');
 const receiptId=hash(coordinator+'\0'+event.id);return store.transaction(()=>{if(store.get('received',receiptId))return false;const item=event.item,id=hash(sourceId+'\0'+item.externalId),old=store.get('items',id);store.put('items',{...item,id,sourceId,read:old?.read||false,saved:old?.saved||false,provenance:{coordinator,eventId:event.id}});store.put('received',{id:receiptId});const oldCursor=store.get('remoteCursors',coordinator)?.cursor||0;store.put('remoteCursors',{id:coordinator,cursor:Math.max(oldCursor,event.seq)});return true;});
}
export async function consumeStream({url,token,store,sourceMap,signal,onEvent=()=>{}}){
 const coordinator=new URL(url).origin;const endpoint=new URL('/api/v3/fleet/transport/stream',coordinator);const cursor=store.get('remoteCursors',coordinator)?.cursor||0;
 const response=await fetch(endpoint,{headers:{authorization:`Bearer ${token}`,'last-event-id':String(cursor)},signal,redirect:'error'});if(!response.ok)throw new Error(`Fleet stream HTTP ${response.status}`);
 let buffer='';const decoder=new TextDecoder();for await(const chunk of response.body){buffer+=decoder.decode(chunk,{stream:true});if(buffer.length>2_000_000)throw new Error('Fleet frame too large');let index;while((index=buffer.indexOf('\n\n'))>=0){const frame=buffer.slice(0,index);buffer=buffer.slice(index+2);const data=frame.split('\n').find(l=>l.startsWith('data: '));if(data){const event=JSON.parse(data.slice(6));applyDelta(store,event,sourceMap,coordinator);onEvent(event);}}}
}
