import {normalizeAtPost} from './threads.mjs';
import {createHash} from 'node:crypto';
export const hash = s => createHash('sha256').update(s).digest('hex');
const decode=s=>s.replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g,'$1').replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/&quot;/g,'"').replace(/&apos;/g,"'").replace(/&amp;/g,'&');
export const plain=s=>decode(String(s??'')).replace(/<[^>]*>/g,' ').replace(/\s+/g,' ').trim();
const tag=(s,n)=>{const v=s.match(new RegExp(`<${n}(?:\\s[^>]*)?>([\\s\\S]*?)<\\/${n}>`,'i'));return v?decode(v[1]):'';};
const attr=(s,n)=>s.match(new RegExp(`\\b${n}=["']([^"']*)["']`,'i'))?.[1]||'';
const safeLink=(s,base)=>{try{const u=new URL(s,base);return ['http:','https:'].includes(u.protocol)&&!u.username&&!u.password?u.href:'';}catch{return '';}};
export function parseFeed(body,url,kind='rss') {
 if(body.length>2_000_000)throw new Error('Feed too large');
 if(/<!DOCTYPE|<!ENTITY/i.test(body))throw new Error('DTD/entity declarations are not supported');
 if(kind!=='atproto'&&(kind==='json'||body.trim().startsWith('{'))) {const d=JSON.parse(body);if(!Array.isArray(d.items))throw new Error('Not a JSON Feed');return d.items.slice(0,500).map(x=>normalize({externalId:x.id,url:safeLink(x.url||x.external_url,url),title:x.title,content:x.content_text||x.content_html||x.summary,publishedAt:x.date_published,author:x.authors?.[0]?.name},url));}
 if(kind==='atproto'){const d=JSON.parse(body);if(!Array.isArray(d.feed))throw new Error('Not an ATProto AppView feed');return d.feed.slice(0,500).map(x=>normalizeAtPost(x.post||x));}
 const entries=[...body.matchAll(/<(item|entry)\b[^>]*>([\s\S]*?)<\/\1>/gi)].slice(0,500);
 if(!entries.length&&!/<(rss|feed|rdf:RDF)\b/i.test(body))throw new Error('Unsupported feed document');
 return entries.map(([,type,s])=>normalize({externalId:tag(s,'guid')||tag(s,'id'),title:tag(s,'title'),url:safeLink(type==='entry'?attr(s.match(/<link\b[^>]*(?:rel=["']alternate["'])?[^>]*>/i)?.[0]||'','href'):tag(s,'link'),url),content:tag(s,'content:encoded')||tag(s,'content')||tag(s,'description')||tag(s,'summary'),publishedAt:tag(s,'pubDate')||tag(s,'published')||tag(s,'updated'),author:tag(s,'dc:creator')||plain(tag(s,'author'))},url));
}
function normalize(x,sourceUrl){return {...x,externalId:String(x.externalId||x.url||hash(String(x.title)+String(x.content))),title:plain(x.title).slice(0,500)||'Untitled',content:plain(x.content).slice(0,100000),author:plain(x.author).slice(0,300),url:x.url||sourceUrl,publishedAt:Number.isFinite(Date.parse(x.publishedAt))?new Date(x.publishedAt).toISOString():new Date().toISOString()};}
export function discover(body,url){return [...body.matchAll(/<link\b[^>]*>/gi)].map(m=>m[0]).filter(t=>/application\/(rss\+xml|atom\+xml|feed\+json)/i.test(attr(t,'type'))).map(t=>({url:safeLink(attr(t,'href'),url),title:plain(attr(t,'title'))||'Discovered feed',kind:attr(t,'type').includes('json')?'json':attr(t,'type').includes('atom')?'atom':'rss'})).filter(x=>x.url).slice(0,20);}
export function classify(text){const rules={Technology:/software|code|rust|javascript|computer|ai\b/i,Science:/science|research|physics|space/i,Design:/design|typography|interface/i};return Object.entries(rules).filter(([,r])=>r.test(text)).map(([k])=>k);}
