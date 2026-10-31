import {createApp} from '../server/app.mjs';
const rootUri='at://did:plc:qafixture/app.bsky.feed.post/root';
const blockedUri='at://did:plc:qafixture/app.bsky.feed.post/blocked';
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
  if(url.includes('error'))throw Error('Deterministic fixture origin failure');
  if(url.includes('empty'))return {url,body:'<html>No feed links here</html>'};
  if(url.includes('website'))return {url,body:'<link rel="alternate" type="application/feed+json" title="QA discovery" href="https://fixture.test/feed.json">'};
  return {url,body:JSON.stringify({version:'https://jsonfeed.org/version/1.1',items:[{id:'qa-one',title:'Science and software fixture',content_text:'A deterministic article for browser verification. No external origin was contacted.',url:'https://fixture.test/article',date_published:'2026-10-01T00:00:00Z'}]})};
 }
});
app.server.listen(8799,'127.0.0.1',()=>console.log('QA fixture server http://127.0.0.1:8799'));
process.on('SIGTERM',async()=>{await app.close();process.exit(0)});
