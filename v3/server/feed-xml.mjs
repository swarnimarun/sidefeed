import sax from './vendor/sax.cjs';
const ATOM='http://www.w3.org/2005/Atom', RSS='http://purl.org/rss/1.0/', RDF='http://www.w3.org/1999/02/22-rdf-syntax-ns#', XML='http://www.w3.org/XML/1998/namespace';
const DC='http://purl.org/dc/elements/1.1/', CONTENT='http://purl.org/rss/1.0/modules/content/';
const attr=(n,name,uri='')=>Object.values(n?.attrs||{}).find(a=>a.local===name&&a.uri===uri)?.value||'';
const children=(n,name,uri)=>n?.children.filter(x=>x.local===name&&(uri===undefined||x.uri===uri))||[];
const child=(n,name,uri)=>children(n,name,uri)[0];
const text=n=>n?n.parts.map(p=>typeof p==='string'?p:text(p)).join(''):'';
function safe(raw,base){if(!raw)return '';try{const u=new URL(raw,base);return ['http:','https:'].includes(u.protocol)&&!u.username&&!u.password?u.href:'';}catch{return '';}}
export function parseXmlFeed(body,sourceUrl){
 if(typeof body!=='string'||Buffer.byteLength(body)>2_000_000)throw new Error('Feed too large');
 if(/<!DOCTYPE|<!ENTITY/i.test(body))throw new Error('DTD/entity declarations are not supported');
 // HTTP transport is UTF-8 only. Reject declarations we cannot faithfully decode.
 const encoding=body.match(/^\s*<\?xml\s[^?]*encoding\s*=\s*["']([^"']+)/i)?.[1];if(encoding&&!/^utf-?8$/i.test(encoding))throw new Error('Only UTF-8 XML feeds are supported');
 const parser=sax.parser(true,{xmlns:true,strictEntities:true,unparsedEntities:false});let root=null,count=0,textSize=0;const stack=[];
 parser.onerror=e=>{throw new Error('Malformed XML feed: '+e.message);};parser.ondoctype=()=>{throw new Error('DTD/entity declarations are not supported');};parser.onsgmldeclaration=()=>{throw new Error('Unsupported XML declaration');};
 parser.onopentag=tag=>{if(++count>20000||stack.length>=64)throw new Error('XML complexity limit exceeded');if(Object.keys(tag.attributes).length>100)throw new Error('XML attribute limit exceeded');const parent=stack.at(-1);const n={local:tag.local,uri:tag.uri,attrs:tag.attributes,children:[],parts:[],base:parent?.base||sourceUrl};const base=attr(n,'base',XML);if(base)n.base=safe(base,n.base)||n.base;if(parent){parent.children.push(n);parent.parts.push(n);}else{if(root)throw new Error('Multiple XML roots');root=n;}stack.push(n);};
 parser.onclosetag=()=>{stack.pop();};const add=s=>{textSize+=s.length;if(textSize>2_000_000)throw new Error('XML text limit exceeded');if(stack.length)stack.at(-1).parts.push(s);};parser.ontext=add;parser.oncdata=add;
 for(let i=0;i<body.length;i+=16384)parser.write(body.slice(i,i+16384));parser.close();if(!root)throw new Error('Empty XML feed');
 let entries,mode,ns;
 if(root.local==='rss'&&!root.uri){mode='rss';ns='';const channel=child(root,'channel','');if(!channel)throw new Error('RSS channel missing');entries=children(channel,'item','');}
 else if(root.local==='RDF'&&root.uri===RDF){mode='rdf';ns=RSS;entries=children(root,'item',RSS);const channel=child(root,'channel',RSS);const seq=child(child(channel,'items',RSS),'Seq',RDF);if(seq){const order=children(seq,'li',RDF).map(n=>safe(attr(n,'resource',RDF),n.base));entries.sort((a,b)=>{const ai=order.indexOf(safe(attr(a,'about',RDF),a.base)),bi=order.indexOf(safe(attr(b,'about',RDF),b.base));return (ai<0?Infinity:ai)-(bi<0?Infinity:bi);});}}
 else if(root.local==='feed'&&(root.uri===ATOM||!root.uri)){mode='atom';ns=root.uri;entries=children(root,'entry',ns);}
 else throw new Error('Unsupported feed document');
 return entries.slice(0,500).map(n=>{
  const field=name=>text(child(n,name,ns));let link;
  if(mode==='atom'){const links=children(n,'link',ns).filter(l=>!attr(l,'rel')||attr(l,'rel')==='alternate');const preferred=links.find(l=>!attr(l,'type')||attr(l,'type')==='text/html')||links[0];link=preferred?safe(attr(preferred,'href'),preferred.base):'';}
  else {const l=child(n,'link',ns);link=safe(text(l),l?.base||n.base);}
  const author=child(n,'author',ns)|| (mode==='atom'?child(root,'author',ns):null);
  return {externalId:mode==='atom'?field('id'):field('guid')||(mode==='rdf'?safe(attr(n,'about',RDF),n.base):''),title:field('title'),url:link,content:text(child(n,'encoded',CONTENT))||field('content')||field('description')||field('summary'),publishedAt:field('pubDate')||field('published')||field('updated')||text(child(n,'date',DC)),author:text(child(n,'creator',DC))||(mode==='atom'?text(child(author,'name',ns)):text(author))};
 });
}
