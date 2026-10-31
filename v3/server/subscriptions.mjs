// Public pull subscriptions, not authenticated federation follows. See SUBSCRIPTIONS.md.
import net from 'node:net';
import {safeFetch,validUrl,publicAddress} from './fetch.mjs';
import {parseFeed,discover,plain} from './adapters.mjs';
import {normalizeAtPost} from './threads.mjs';
const APP='https://public.api.bsky.app/xrpc/';
const PUBLIC='https://www.w3.org/ns/activitystreams#Public';
const ACCEPT='application/activity+json, application/ld+json; profile="https://www.w3.org/ns/activitystreams", application/json';
const didOK=v=>typeof v==='string'&&/^did:(plc:[a-z2-7]{24}|web:[A-Za-z0-9._:%-]+)$/.test(v);
const id=v=>typeof v==='string'?v:v?.id;
function url(raw,base){const u=validUrl(base?new URL(raw,base).href:raw);const h=u.hostname.replace(/^\[|\]$/g,'');if(h==='localhost'||h.endsWith('.localhost')||net.isIP(h)&&!publicAddress(h))throw new Error('Non-public network targets are blocked');return u.href;}
function authority(raw,base){const u=url(raw,base);if(new URL(u).origin!==new URL(base).origin)throw new Error('Cross-origin actor/outbox links are unsupported');return u;}
function json(r){if(typeof r.body!=='string'||Buffer.byteLength(r.body)>2_000_000)throw new Error('Protocol response exceeds size limit');const d=JSON.parse(r.body);if(!d||typeof d!=='object'||Array.isArray(d))throw new Error('Invalid protocol document');return d;}
async function get(fetcher,u,headers={},base){u=base?authority(u,base):url(u);const r=await fetcher(u,{maxBytes:2_000_000,headers,allowStatuses:[304,429,503]});if(r.url){url(r.url);if(base)authority(r.url,base);}if(typeof r.body!=='string'||Buffer.byteLength(r.body)>2_000_000)throw new Error('Protocol response exceeds size limit');return r;}
function success(r){if(r.status<200||r.status>=300)throw new Error(`Origin HTTP ${r.status}`);return r;}
function atURL(actor,cursor){const u=new URL(APP+'app.bsky.feed.getAuthorFeed');u.searchParams.set('actor',actor);u.searchParams.set('limit','100');u.searchParams.set('filter','posts_with_replies');u.searchParams.set('includePins','false');if(cursor)u.searchParams.set('cursor',cursor);return u.href;}
export async function previewSubscription({input,kind='rss'},{fetcher=safeFetch,now=Date.now()}={}){
 if(typeof input!=='string'||!input.trim()||input.length>2048)throw new Error('A source URL or public profile is required');input=input.trim();let source;
 if(kind==='atproto'){
  let actor=input.replace(/^@/,'');if(/^https?:/.test(actor)){const u=new URL(url(actor));if(u.origin!=='https://bsky.app'||!/^\/profile\/[^/]+\/?$/.test(u.pathname))throw new Error('Use a Bluesky profile URL, handle or DID');actor=decodeURIComponent(u.pathname.split('/')[2]);}
  let handle=null;if(!didOK(actor)){if(!/^(?=.{3,253}$)[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?\.[a-z]{2,}$/i.test(actor))throw new Error('Invalid Bluesky handle or DID');handle=actor;const u=new URL(APP+'com.atproto.identity.resolveHandle');u.searchParams.set('handle',actor);actor=json(success(await get(fetcher,u.href,{accept:'application/json'}))).did;if(!didOK(actor))throw new Error('Invalid resolved DID');}
  source={url:atURL(actor),kind,title:handle||actor,subscription:{protocol:kind,actorId:actor,handle,identityVerification:'AppView handle resolution; not independent DID verification'}};
 }else if(kind==='activitypub'){
  let actorUrl=input;const acct=input.replace(/^acct:/,'').replace(/^@/,'');
  if(!/^https?:/.test(input)){
   const match=acct.match(/^([^@\s/?#]+)@([^@\s/?#]+)$/);if(!match)throw new Error('Use acct@host or an ActivityPub actor URL');const origin=new URL(url('https://'+match[2]));if(origin.port)throw new Error('Account host ports are unsupported');const resource='acct:'+acct;const wf=new URL('/.well-known/webfinger',origin);wf.searchParams.set('resource',resource);const d=json(success(await get(fetcher,wf.href,{accept:'application/jrd+json'},origin.href)));if(d.subject!==resource)throw new Error('WebFinger subject mismatch');actorUrl=d.links?.find(l=>l.rel==='self'&&['application/activity+json','application/ld+json; profile="https://www.w3.org/ns/activitystreams"'].includes(l.type))?.href;if(!actorUrl)throw new Error('No ActivityPub self link');actorUrl=authority(actorUrl,origin.href);
  }
  actorUrl=url(actorUrl);const r=success(await get(fetcher,actorUrl,{accept:ACCEPT},actorUrl));const actor=json(r);if(!['Person','Service','Organization','Application','Group'].includes(actor.type)||typeof actor.id!=='string'||!actor.outbox)throw new Error('Unsupported ActivityPub actor');const actorId=authority(actor.id,actorUrl);if(actorId!==(r.url||actorUrl))throw new Error('Actor identity does not match fetched URL');const outbox=authority(id(actor.outbox),actorId);source={url:actorId,kind,title:plain(actor.name||actor.preferredUsername)||actorId,subscription:{protocol:kind,actorId,outbox}};
 }else if(['rss','atom','json'].includes(kind)){
  let r=success(await get(fetcher,url(input)));let feedUrl=r.url||url(input);let discovered=[];
  if(/<html\b|<!doctype\s+html/i.test(r.body)){discovered=discover(r.body,feedUrl);if(!discovered.length)throw new Error('No RSS/Atom/JSON feed discovered; choose an explicit public profile protocol');const first=discovered[0];kind=first.kind;feedUrl=url(first.url);r=success(await get(fetcher,feedUrl));feedUrl=r.url||feedUrl;}
  const items=parseFeed(r.body,feedUrl,kind);source={url:feedUrl,kind,title:discovered[0]?.title||new URL(feedUrl).hostname,subscription:{protocol:kind}};
  return {source,items:items.map(x=>({...x,provenance:{protocol:kind,sourceUrl:feedUrl,publicPull:true}})),poll:pollMeta(r,{},now),warnings:['Bounded RSS/Atom extraction using vendored sax; UTF-8 only, not full feed schema validation.'],discovered};
 }else throw new Error('Unsupported subscription protocol');
 return {source,...await pollSubscription(source,{fetcher,now})};
}
function pollMeta(r,old,now){return {...old,etag:r.headers?.etag||null,lastModified:r.headers?.['last-modified']||null,failures:0,lastError:null,lastPolledAt:new Date(now).toISOString(),nextPollAt:new Date(now+300_000).toISOString(),cursor:null};}
export function pollFailure(old={},error,now=Date.now(),retryAfter){const failures=Math.min(10,(old.failures||0)+1);let delay=Math.min(86400,300*2**(failures-1));if(retryAfter){const seconds=/^\d+$/.test(retryAfter)?Number(retryAfter):(Date.parse(retryAfter)-now)/1000;if(Number.isFinite(seconds))delay=Math.min(86400,Math.max(delay,seconds));}return {...old,failures,lastError:String(error?.message||error).slice(0,500),nextPollAt:new Date(now+delay*1000).toISOString()};}
function apItems(entries,sub,sourceUrl){const out=[];for(const entry of entries){if(!entry||typeof entry!=='object')continue;const activity=['Create','Update','Delete'].includes(entry.type);if(activity&&id(entry.actor)!==sub.actorId)continue;const obj=entry.type==='Delete'&&typeof entry.object==='string'?{id:entry.object,type:'Tombstone'}:activity?entry.object:entry;if(!obj||typeof obj!=='object')continue;const publicTo=x=>[...(Array.isArray(x.to)?x.to:[x.to]),...(Array.isArray(x.cc)?x.cc:[x.cc])].some(v=>[PUBLIC,'as:Public','Public'].includes(v));if(('to' in obj||'cc' in obj)?!publicTo(obj):!publicTo(entry))continue;if(id(obj.attributedTo)!==sub.actorId&&!(activity&&entry.type==='Delete'))continue;if(typeof obj.id!=='string')continue;let canonical;try{canonical=authority(obj.id,sub.actorId);}catch{continue;}
 const tombstone=obj.type==='Tombstone'||entry.type==='Delete';if(!tombstone&&!['Note','Article','Page'].includes(obj.type))continue;
 let link=canonical;try{if(typeof obj.url==='string')link=url(obj.url);}catch{}
 out.push({externalId:canonical,uri:canonical,state:tombstone?'deleted':'post',tombstone,author:sub.actorId,authorId:sub.actorId,title:tombstone?'Deleted post':plain(obj.name||obj.summary||obj.content).slice(0,500)||'Untitled',content:tombstone?'':plain(obj.content).slice(0,100000),url:link,publishedAt:Number.isFinite(Date.parse(obj.published))?new Date(obj.published).toISOString():null,reply:obj.inReplyTo?{parent:{uri:id(obj.inReplyTo)}}:null,provenance:{protocol:'activitypub',sourceUrl,actorId:sub.actorId,activityId:entry.id||null,publicPull:true}});
 }return out;}
export async function pollSubscription(source,{fetcher=safeFetch,poll={},now=Date.now(),maxPages=2,backfill=false}={}){
 maxPages=Math.max(1,Math.min(5,Number.isInteger(maxPages)?maxPages:2));const sub=source.subscription||{protocol:source.kind};const protocol=sub.protocol;const headers={accept:protocol==='activitypub'?ACCEPT:protocol==='atproto'?'application/json':'application/feed+json, application/atom+xml, application/rss+xml'};
 if(!backfill){if(poll.etag)headers['if-none-match']=poll.etag;if(poll.lastModified)headers['if-modified-since']=poll.lastModified;}
 let target=protocol==='atproto'?atURL(sub.actorId,backfill?poll.cursor:null):protocol==='activitypub'?(backfill&&poll.cursor||sub.outbox):source.url;
 const items=new Map(),seen=new Set();let first=null,cursor=null,pages=0,embedded=null;
 try{
 while(target&&pages<maxPages){if(seen.has(target))break;seen.add(target);const r=embedded?{body:JSON.stringify(embedded),status:200,headers:{},url:target}:await get(fetcher,target,pages?{accept:headers.accept}:headers,protocol==='activitypub'?sub.actorId:undefined);embedded=null;pages++;first||=r;
 if(r.status===304)return {items:[],notModified:true,poll:{...poll,failures:0,lastError:null,lastPolledAt:new Date(now).toISOString(),nextPollAt:new Date(now+300_000).toISOString()},warnings:[]};
 if([429,503].includes(r.status))return {items:[],poll:pollFailure(poll,`Origin HTTP ${r.status}`,now,r.headers?.['retry-after']),warnings:['Origin requested backoff'],error:`Origin HTTP ${r.status}`};success(r);
 if(protocol==='atproto'){if(!didOK(sub.actorId))throw new Error('Invalid canonical DID');const d=json(r);if(!Array.isArray(d.feed)||d.feed.length>500)throw new Error('Invalid author feed');for(const f of d.feed){const post=normalizeAtPost(f.post);if(post.authorDid!==sub.actorId)continue;items.set(post.externalId,{...post,provenance:{protocol,sourceUrl:r.url||target,actorId:sub.actorId,publicPull:true}});}cursor=typeof d.cursor==='string'&&d.cursor.length<=4096?d.cursor:null;target=cursor?atURL(sub.actorId,cursor):null;
 }else if(protocol==='activitypub'){const d=json(r);if(!['OrderedCollection','OrderedCollectionPage','Collection','CollectionPage'].includes(d.type))throw new Error('Invalid public outbox collection');if(d.partOf&&id(d.partOf)!==sub.outbox)throw new Error('Outbox page authority mismatch');const entries=d.orderedItems||d.items||[];if(!Array.isArray(entries)||entries.length>500)throw new Error('Invalid outbox items');for(const x of apItems(entries,sub,r.url||target))items.set(x.externalId,x);const next=d.next||(!entries.length?d.first:null);const nextId=id(next);cursor=nextId?authority(nextId,sub.actorId):null;if(next&&typeof next==='object'&&cursor)embedded=next;target=cursor;
 }else {for(const x of parseFeed(r.body,r.url||target,protocol))items.set(x.externalId,{...x,provenance:{protocol,sourceUrl:r.url||target,publicPull:true}});target=null;}
 }
 return {items:[...items.values()],poll:{...pollMeta(first,poll,now),cursor},notModified:false,warnings:protocol==='activitypub'?['Public same-origin outbox subset only; no federation Follow or authenticated access.']:protocol==='atproto'?['Public AppView author posts only; reposts excluded.']:['Bounded feed extraction; not full RSS/Atom schema validation.'],partial:!!target};
 }catch(error){error.poll=pollFailure(poll,error,now);throw error;}
}
