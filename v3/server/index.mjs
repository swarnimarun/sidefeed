import {mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {createApp} from './app.mjs';
const host=process.env.HOST||'127.0.0.1';const port=Number(process.env.PORT||8787);const token=process.env.SIDEFEED_TOKEN||'';
if(!['127.0.0.1','::1','localhost'].includes(host)&&token.length<24)throw new Error('Remote binding requires a SIDEFEED_TOKEN of at least 24 characters and a TLS reverse proxy');
const dbPath=resolve(process.env.SIDEFEED_DB||'data/sidefeed.sqlite');mkdirSync(dirname(dbPath),{recursive:true,mode:0o700});
const app=createApp({dbPath,token});app.server.listen(port,host,()=>console.log(`Sidefeed V3 listening on http://${host}:${port}`));
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,async()=>{await app.close();process.exit(0);});
