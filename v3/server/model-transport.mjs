import dns from 'node:dns/promises';
import https from 'node:https';
import net from 'node:net';
import {publicAddress,validUrl} from './fetch.mjs';
// POST-only provider transport: pinned public DNS, HTTPS, no redirects or credential forwarding.
export async function providerRequest(url,{key,body,signal,timeout=20000}){
 const u=validUrl(url);if(u.protocol!=='https:')throw new Error('Model endpoint requires HTTPS');
 const host=u.hostname.replace(/^\[|\]$/g,'');let dnsTimer;let addresses;try{addresses=net.isIP(host)?[{address:host,family:net.isIP(host)}]:await Promise.race([dns.lookup(host,{all:true}),new Promise((_,reject)=>{dnsTimer=setTimeout(()=>reject(new Error('Provider DNS deadline exceeded')),5000);})]);}finally{clearTimeout(dnsTimer);}
 if(signal?.aborted)throw new Error('Canceled');if(!addresses.length||addresses.some(a=>!publicAddress(a.address)))throw new Error('Non-public model endpoints blocked');
 const chosen=addresses[0],payload=JSON.stringify(body);
 return new Promise((resolve,reject)=>{const req=https.request(u,{method:'POST',signal,headers:{'content-type':'application/json','content-length':Buffer.byteLength(payload),'accept':'application/json','accept-encoding':'identity',...(key?{authorization:`Bearer ${key}`}:{})},lookup:(_h,o,cb)=>o?.all?cb(null,[chosen]):cb(null,chosen.address,chosen.family)},res=>{let size=0;const chunks=[];res.on('data',c=>{size+=c.length;if(size>4_000_000)req.destroy(new Error('Provider response exceeds limit'));else chunks.push(c);});res.on('error',reject);res.on('end',()=>{if(res.statusCode<200||res.statusCode>=300)return reject(new Error(`Provider HTTP ${res.statusCode}; redirects are not followed`));try{resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')));}catch{reject(new Error('Invalid provider JSON'));}});});const timer=setTimeout(()=>req.destroy(new Error('Provider deadline exceeded')),timeout);req.on('close',()=>clearTimeout(timer));req.on('error',reject);req.end(payload);});
}
