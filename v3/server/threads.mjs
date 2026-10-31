// Bounded public AppView normalization. No network, authentication or stream state.
const POST = /^at:\/\/(did:[a-z0-9]+:[A-Za-z0-9._:%-]+)\/app\.bsky\.feed\.post\/([A-Za-z0-9._~:-]{1,512})$/;
export function postIdentity(uri) {
 const match=typeof uri==='string' && uri.length<=2048 && uri.match(POST);
 if(!match || match[2]==='.' || match[2]==='..') throw new Error('Expected a DID-based ATProto post URI');
 return {uri,did:match[1],rkey:match[2]};
}
const text=(v,n)=>String(v??'').replace(/<[^>]*>/g,' ').replace(/\s+/g,' ').trim().slice(0,n);
const ref=v=>v?{uri:postIdentity(v.uri).uri,cid:typeof v.cid==='string'?v.cid.slice(0,256):null}:null;
export function normalizeAtPost(post) {
 if(!post || typeof post!=='object') throw new Error('Invalid ATProto post');
 const id=postIdentity(post.uri), record=post.record||{};
 const reply=record.reply?{parent:ref(record.reply.parent),root:ref(record.reply.root)}:null;
 return {externalId:id.uri,uri:id.uri,cid:typeof post.cid==='string'?post.cid.slice(0,256):null,
  state:'post',reply,authorDid:id.did,author:text(post.author?.handle||id.did,300),
  title:text(record.text,500)||'Untitled',content:text(record.text,100000),
  url:`https://bsky.app/profile/${encodeURIComponent(id.did)}/post/${encodeURIComponent(id.rkey)}`,
  publishedAt:Number.isFinite(Date.parse(record.createdAt))?new Date(record.createdAt).toISOString():null};
}
function limit(value,fallback,max) {
 if(value===undefined)return fallback;
 if(!Number.isInteger(value)||value<0||value>max)throw new Error('Invalid thread traversal limit');
 return value;
}
export function threadUrl(uri,{depth,parentHeight}={}) {
 postIdentity(uri);
 const u=new URL('https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread');
 u.searchParams.set('uri',uri);u.searchParams.set('depth',limit(depth,6,12));u.searchParams.set('parentHeight',limit(parentHeight,6,12));
 return u.href;
}
export function normalizeThread(response,{maxNodes,maxDepth}={}) {
 maxNodes=limit(maxNodes,200,500);maxDepth=limit(maxDepth,12,24);
 if(maxNodes<1)throw new Error('maxNodes must be positive');
 const root=response?.thread;
 if(!root||typeof root!=='object')throw new Error('Missing AppView thread');
 const rootUri=postIdentity(root.post?.uri||root.uri).uri;
 const nodes=new Map(), edges=new Map(),reasons=new Set(['bounded-appview-snapshot']);
 let truncated=false;
 const clipped=()=>{truncated=true;reasons.add('traversal-limit');};
 function add(node){
  if(!nodes.has(node.uri)&&nodes.size>=maxNodes){clipped();return false;}
  const old=nodes.get(node.uri);
  if(!old || old.state==='missing' || node.state==='post')nodes.set(node.uri,node);
  return true;
 }
 function placeholder(uri,cid=null,state='missing') {return {uri,externalId:uri,cid,state};}
 function edge(from,to,type,cid=null){
  if(!nodes.has(to)&&!add(placeholder(to,cid)))return;
  if(nodes.has(from)&&nodes.has(to))edges.set(`${from}\n${type}\n${to}`,{from,to,type});
 }
 const stack=[{value:root,depth:0,child:null,parent:null}];
 const seen=new Set(); let visited=0;
 while(stack.length){
  if(++visited>maxNodes*4){clipped();break;}
  const {value,depth,child,parent}=stack.pop();
  if(!value||typeof value!=='object')throw new Error('Invalid thread node');
  const uri=postIdentity(value.post?.uri||value.uri).uri;
  const state=value.post?'post':value.blocked||value.$type?.endsWith('#blockedPost')?'blocked':value.notFound||value.$type?.endsWith('#notFoundPost')?'not-found':'missing';
  if(!add(value.post?normalizeAtPost(value.post):placeholder(uri,null,state)))continue;
  if(child)edge(child,uri,'reply');if(parent)edge(uri,parent,'reply');
  const node=nodes.get(uri);
  if(node.reply?.parent)edge(uri,node.reply.parent.uri,'reply',node.reply.parent.cid);
  if(node.reply?.root)edge(uri,node.reply.root.uri,'root',node.reply.root.cid);
  if(seen.has(uri))continue;seen.add(uri);
  if(state!=='post'){reasons.add(state);continue;}
  const replies=Array.isArray(value.replies)?value.replies:[];
  if(depth>=maxDepth){if(value.parent||replies.length)clipped();continue;}
  // Queue is independently capped: a broad response cannot grow unbounded work.
  const available=Math.max(0,maxNodes*2-stack.length);
  if(replies.length>available)clipped();
  for(const reply of replies.slice(0,available).reverse())stack.push({value:reply,depth:depth+1,parent:uri,child:null});
  if(value.parent){if(stack.length<maxNodes*2)stack.push({value:value.parent,depth:depth+1,child:uri,parent:null});else clipped();}
 }
 return {rootUri,nodes:[...nodes.values()],edges:[...edges.values()],partial:true,truncated,reasons:[...reasons]};
}
