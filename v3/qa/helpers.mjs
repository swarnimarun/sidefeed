import {createApp} from '../server/app.mjs';
export async function start(options={}) {
 const app=createApp(options);await new Promise(resolve=>app.server.listen(0,'127.0.0.1',resolve));
 const base=`http://127.0.0.1:${app.server.address().port}`;
 return {...app,base,async api(path,method='GET',body,token,extra={}) {
  const response=await fetch(base+path,{method,headers:{'content-type':'application/json',...(token?{authorization:`Bearer ${token}`} :{}),...extra},body:body===undefined?undefined:JSON.stringify(body)});
  return {status:response.status,body:await response.json()};
 }};
}
export const article={externalId:'fixture-one',title:'Research fixture',content:'Research on software systems',url:'https://fixture.test/article',author:'Fixture Author',publishedAt:'2026-10-01T00:00:00Z'};
export function seed(app) {const source=app.store.create('sources',{title:'QA Source',url:'https://fixture.test/feed',kind:'json',paused:false,share:false});const item=app.store.create('items',{...article,sourceId:source.id,read:true,saved:true});return {source,item};}
