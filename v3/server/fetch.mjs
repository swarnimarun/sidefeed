import dns from 'node:dns/promises';
import https from 'node:https';
import http from 'node:http';
import net from 'node:net';
export function publicAddress(ip) {
  if (net.isIPv4(ip)) { const [a,b] = ip.split('.').map(Number); return !(a===0||a===10||a===127||a>=224||a===169&&b===254||a===172&&b>=16&&b<=31||a===192&&b===168||a===100&&b>=64&&b<=127||a===198&&(b===18||b===19||b===51)||a===192&&(b===0||b===88)||a===203&&b===0); }
  // Conservative IPv6: global unicast only, excluding mapped IPv4 and documentation.
  return net.isIPv6(ip) && /^[23]/.test(ip) && !/^2001:(db8|0):/i.test(ip) && !/^2002:/i.test(ip) && !ip.includes('.');
}
export function validUrl(raw) { const u = new URL(raw); if (!['http:','https:'].includes(u.protocol)||u.username||u.password) throw new Error('Only credential-free HTTP(S) URLs are allowed'); u.hash=''; return u; }
export async function safeFetch(raw, {maxBytes=2_000_000,timeout=8000, redirects=3}={}) {
 const u=validUrl(raw); const host=u.hostname.replace(/^\[|\]$/g,'');
 const addresses=net.isIP(host)?[{address:host,family:net.isIP(host)}]:await dns.lookup(host,{all:true});
 if (!addresses.length || addresses.some(a=>!publicAddress(a.address))) throw new Error('Non-public network targets are blocked');
 const chosen=addresses[0];
 const result=await new Promise((resolve,reject)=>{
  const req=(u.protocol==='https:'?https:http).get(u,{headers:{'user-agent':'Sidefeed-V3/0.1','accept':'application/feed+json, application/atom+xml, application/rss+xml, text/html;q=0.8','accept-encoding':'identity'},lookup:(_h,_o,cb)=>_o?.all?cb(null,[chosen]):cb(null,chosen.address,chosen.family)},res=>{
   let size=0; const chunks=[];
   res.on('data',chunk=>{size+=chunk.length;if(size>maxBytes)req.destroy(new Error('Response exceeds size limit'));else chunks.push(chunk);});
   res.on('end',()=>resolve({status:res.statusCode,headers:res.headers,body:Buffer.concat(chunks).toString('utf8'),url:u.href})); res.on('error',reject);
  }); const timer=setTimeout(()=>req.destroy(new Error('Fetch deadline exceeded')),timeout);req.on('close',()=>clearTimeout(timer));req.on('error',reject);
 });
 if ([301,302,303,307,308].includes(result.status)) {if(!redirects||!result.headers.location)throw new Error('Redirect limit exceeded');return safeFetch(new URL(result.headers.location,u).href,{maxBytes,timeout,redirects:redirects-1});}
 if(result.status<200||result.status>=300)throw new Error(`Origin HTTP ${result.status}`);
 return result;
}
