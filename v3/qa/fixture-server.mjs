import {createApp} from '../server/app.mjs';
const rootUri='at://did:plc:qafixture/app.bsky.feed.post/root';
const profileDID='did:plc:abcdefghijklmnopqrstuvwx';
const profileActor='https://social.fixture.test/users/qa';
const blockedUri='at://did:plc:qafixture/app.bsky.feed.post/blocked';
const fixtureFeed={version:'https://jsonfeed.org/version/1.1',items:[
 {id:'qa-one',title:'Science and software fixture',content_text:'A deterministic article for browser verification. No external origin was contacted.\n\nSmall research groups are building tools that make public knowledge easier to follow. This synthetic story explores a quieter approach: clear sources, durable reading lists, and useful context beside the article.',url:'https://fixture.test/article',authors:[{name:'The Fixture Review'}],date_published:'2026-10-03T09:00:00Z'},
 {id:'qa-two',title:'Designing a slower, more useful reading list',content_text:'This synthetic design report asks what happens when a reader has fewer panels and better typography. Short summaries help readers choose; full articles and original sources remain one deliberate action away.\n\nThe goal is not more time on screen, but a better path through the material already saved.',url:'https://fixture.test/reading-list',authors:[{name:'Design Notebook'}],date_published:'2026-10-02T09:00:00Z'},
 {id:'qa-three',title:'How small research teams share their sources',content_text:'A synthetic science dispatch about shared reading. A useful source list records where a claim came from, why it was selected, and what is still unknown.\n\nRelated articles here are connected by their actual source or deterministic keyword tags, not an undisclosed recommendation model.',url:'https://fixture.test/research',authors:[{name:'Research Dispatch'}],date_published:'2026-10-01T09:00:00Z'}
]};
const app=createApp({
 dbPath:process.env.QA_DB||':memory:',
 // Browser QA is deterministic even if the calling shell has provider environment variables.
 modelsOptions:{config:{endpoint:'',key:'',provider:'QA disabled fixture',classification:'',embedding:'',maxRequestsPerDay:1,maxItems:5,maxInputChars:12000,maxOutputTokens:500},transport:async()=>{throw new Error('QA forbids model provider calls');}},
 fetcher:async url=>{
  const target=new URL(url);
  if(target.origin==='https://public.api.bsky.app'&&target.pathname==='/xrpc/app.bsky.feed.getPostThread'){
   if(target.searchParams.get('uri')!==rootUri)throw Error('Unexpected QA thread identity');
   return {url,body:JSON.stringify({thread:{post:{uri:rootUri,cid:'qa-root-cid',author:{did:'did:plc:qafixture',handle:'qa.fixture.test'},record:{text:'QA public thread root. Synthetic fixture, no external network.',createdAt:'2026-10-01T00:00:00Z'}},replies:[{uri:blockedUri,blocked:true,$type:'app.bsky.feed.defs#blockedPost'}]}})};
  }
  if(target.origin==='https://public.api.bsky.app'&&target.pathname==='/xrpc/com.atproto.identity.resolveHandle')return {url,body:JSON.stringify({did:profileDID})};
  if(target.origin==='https://public.api.bsky.app'&&target.pathname==='/xrpc/app.bsky.feed.getAuthorFeed')return {url,body:JSON.stringify({feed:[{post:{uri:`at://${profileDID}/app.bsky.feed.post/profile`,cid:'qa-profile-cid',author:{did:profileDID,handle:'qa.fixture.test'},record:{text:'Synthetic AT Protocol profile post — fixture only.',createdAt:'2026-10-01T00:00:00Z'}}}]})};
  if(target.origin==='https://social.fixture.test'){
   if(target.pathname==='/.well-known/webfinger')return {url,body:JSON.stringify({subject:'acct:qa@social.fixture.test',links:[{rel:'self',type:'application/activity+json',href:profileActor}]})};
   if(url===profileActor)return {url,body:JSON.stringify({id:profileActor,type:'Person',name:'Synthetic ActivityPub fixture',outbox:profileActor+'/outbox'})};
   if(url===profileActor+'/outbox')return {url,body:JSON.stringify({id:url,type:'OrderedCollection',orderedItems:[{id:profileActor+'/posts/one',type:'Note',attributedTo:profileActor,to:['https://www.w3.org/ns/activitystreams#Public'],content:'Synthetic ActivityPub public post — fixture only.'}]})};
   throw Error('Unexpected synthetic ActivityPub URL');
  }
  if(target.origin!=='https://fixture.test')throw Error('QA forbids unknown external origins');
  if(url.includes('error'))throw Error('Deterministic fixture origin failure');
  if(url.includes('empty'))return {url,body:'<html>No feed links here</html>'};
  if(url.includes('website'))return {url,body:'<link rel="alternate" type="application/feed+json" title="QA discovery" href="https://fixture.test/feed.json">'};
  return {url,body:JSON.stringify(fixtureFeed)};
 }
});
app.server.listen(8799,'127.0.0.1',()=>console.log('QA fixture server http://127.0.0.1:8799'));
// Separate synthetic protected node verifies token lifetime and model consent UI.
// Its transport returns fixture output only; no provider network is possible.
const protectedApp=createApp({token:'qa-browser-owner-token-not-a-real-secret',fetcher:async url=>({url,body:JSON.stringify(fixtureFeed)}),modelsOptions:{config:{endpoint:'https://mock-provider.invalid/v1',key:'',provider:'Synthetic QA provider — no network',classification:'fixture-classifier',summary:'fixture-summary',embedding:'',maxRequestsPerDay:12,maxItems:5,maxInputChars:12000,maxOutputTokens:500},transport:async(_url,{body})=>{
 if(body.model==='fixture-classifier')return {model:'fixture-classifier',choices:[{message:{content:'{"labels":["Fixture"]}'}}]};
 const results=body.messages.filter(m=>m.role==='user').flatMap(m=>{try{return JSON.parse(m.content).tool_result||[];}catch{return [];}});
 let next;
 if(!results.length)next={tool:'search',query:'science'};
 else if(results.length===1)next={tool:'read_items',ids:results[0].items.map(i=>i.id).slice(0,1)};
 else next={tool:'finalize',summary:'Synthetic QA answer: small research groups use clear sources and durable reading lists. This fixture does not contact a live model.',citations:results[1].items.map(i=>i.id)};
 return {model:'fixture-summary',choices:[{message:{content:JSON.stringify(next)}}]};
}}});
const protectedSource=protectedApp.store.create('sources',{title:'Protected synthetic source',url:'https://fixture.test/feed.json',kind:'json',paused:false,share:false});
await protectedApp.refresh(protectedSource);
protectedApp.server.listen(8800,'127.0.0.1',()=>console.log('Protected QA fixture http://127.0.0.1:8800 (synthetic credentials only)'));
process.on('SIGTERM',async()=>{await Promise.all([app.close(),protectedApp.close()]);process.exit(0)});
