// All retrieval is local and indexed unless an explicitly consented provider is injected.
const bad = message => { throw Object.assign(new Error(message), {status:400,code:'invalid_search'}); };
export function compileQuery(q='') {
 if(typeof q!=='string'||q.length>1000)bad('Search query must be at most 1000 characters');
 const groups=[];const re=/"([^"]*)"|([^\s"]+)/gu;let m;
 while((m=re.exec(q))){const tokens=(m[1]??m[2]).normalize('NFKC').match(/[\p{L}\p{N}]+/gu)||[];if(tokens.length)groups.push(tokens.join(' '));}
 if(groups.length>32)bad('At most 32 search terms are allowed');
 return {groups,match:groups.map(s=>'"'+s.replaceAll('"','""')+'"').join(' AND ')};
}
export function initializeSearch(db){
 db.exec(`CREATE TABLE IF NOT EXISTS search_migrations(version INTEGER PRIMARY KEY);
 CREATE TABLE IF NOT EXISTS search_documents(id TEXT PRIMARY KEY,sourceId TEXT,protocol TEXT,publishedAt TEXT,read INTEGER,saved INTEGER,title TEXT,content TEXT,author TEXT);
 CREATE INDEX IF NOT EXISTS search_source ON search_documents(sourceId,publishedAt,id);
 CREATE INDEX IF NOT EXISTS search_protocol ON search_documents(protocol,publishedAt,id);
 CREATE INDEX IF NOT EXISTS search_date ON search_documents(publishedAt,id);
 CREATE INDEX IF NOT EXISTS search_read ON search_documents(read,publishedAt,id);
 CREATE INDEX IF NOT EXISTS search_saved ON search_documents(saved,publishedAt,id);
 CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(title,content,author,content='search_documents',content_rowid='rowid',tokenize='unicode61 remove_diacritics 2');
 CREATE VIRTUAL TABLE IF NOT EXISTS search_vocab USING fts5vocab(search_fts,'row');
 CREATE TRIGGER IF NOT EXISTS search_doc_insert AFTER INSERT ON search_documents BEGIN INSERT INTO search_fts(rowid,title,content,author) VALUES(new.rowid,new.title,new.content,new.author); END;
 CREATE TRIGGER IF NOT EXISTS search_doc_delete AFTER DELETE ON search_documents BEGIN INSERT INTO search_fts(search_fts,rowid,title,content,author) VALUES('delete',old.rowid,old.title,old.content,old.author); END;
 CREATE TRIGGER IF NOT EXISTS search_doc_update AFTER UPDATE ON search_documents BEGIN INSERT INTO search_fts(search_fts,rowid,title,content,author) VALUES('delete',old.rowid,old.title,old.content,old.author); INSERT INTO search_fts(rowid,title,content,author) VALUES(new.rowid,new.title,new.content,new.author); END;`);
 const projection=`new.id,coalesce(json_extract(new.data,'$.sourceId'),''),coalesce((SELECT json_extract(data,'$.kind') FROM records WHERE kind='sources' AND id=json_extract(new.data,'$.sourceId')),json_extract(new.data,'$.protocol'),''),coalesce(json_extract(new.data,'$.publishedAt'),''),coalesce(json_extract(new.data,'$.read'),0),coalesce(json_extract(new.data,'$.saved'),0),coalesce(json_extract(new.data,'$.title'),''),coalesce(json_extract(new.data,'$.content'),''),coalesce(json_extract(new.data,'$.author'),'')`;
 for(const action of ['INSERT','UPDATE'])db.exec(`CREATE TRIGGER IF NOT EXISTS search_item_${action.toLowerCase()} AFTER ${action} ON records WHEN new.kind='items' BEGIN INSERT INTO search_documents VALUES(${projection}) ON CONFLICT(id) DO UPDATE SET sourceId=excluded.sourceId,protocol=excluded.protocol,publishedAt=excluded.publishedAt,read=excluded.read,saved=excluded.saved,title=excluded.title,content=excluded.content,author=excluded.author; END;`);
 db.exec(`CREATE TRIGGER IF NOT EXISTS search_item_delete AFTER DELETE ON records WHEN old.kind='items' BEGIN DELETE FROM search_documents WHERE id=old.id; END;
 CREATE TRIGGER IF NOT EXISTS search_source_update AFTER UPDATE ON records WHEN new.kind='sources' BEGIN UPDATE search_documents SET protocol=coalesce(json_extract(new.data,'$.kind'),'') WHERE sourceId=new.id; END;
 CREATE TRIGGER IF NOT EXISTS search_source_insert AFTER INSERT ON records WHEN new.kind='sources' BEGIN UPDATE search_documents SET protocol=coalesce(json_extract(new.data,'$.kind'),'') WHERE sourceId=new.id; END;`);
 if(!db.prepare('SELECT 1 FROM search_migrations WHERE version=1').get()){
  db.exec('BEGIN IMMEDIATE');try{db.exec("UPDATE records SET data=data WHERE kind='items'; INSERT INTO search_migrations VALUES(1);");db.exec('COMMIT');}catch(e){db.exec('ROLLBACK');throw e;}
 }
 initializeDictionary(db);
}
function editDistance(a,b){const aa=[...a],bb=[...b],rows=[Array.from({length:bb.length+1},(_,i)=>i)];for(let i=1;i<=aa.length;i++){const row=[i];for(let j=1;j<=bb.length;j++){row[j]=Math.min(row[j-1]+1,rows[i-1][j]+1,rows[i-1][j-1]+(aa[i-1]===bb[j-1]?0:1));if(i>1&&j>1&&aa[i-1]===bb[j-2]&&aa[i-2]===bb[j-1])row[j]=Math.min(row[j],rows[i-2][j-2]+1);}rows.push(row);}return rows[aa.length][bb.length];}
function options(o){const limit=Number(o.limit??30),offset=Number(o.offset??0);if(!Number.isInteger(limit)||limit<1||limit>100||!Number.isInteger(offset)||offset<0||offset>10000)bad('limit must be 1–100 and offset 0–10000');const where=[],params=[];
 for(const k of ['sourceId','protocol'])if(o[k]!==undefined&&o[k]!==''){if(typeof o[k]!=='string'||o[k].length>300)bad('Invalid '+k);where.push(`d.${k}=?`);params.push(o[k]);}
 for(const k of ['read','saved'])if(o[k]!==undefined){if(![true,false,'true','false'].includes(o[k]))bad('Invalid '+k);where.push(`d.${k}=?`);params.push(o[k]===true||o[k]==='true'?1:0);}
 for(const [k,op] of [['after','>='],['before','<=']])if(o[k]){if(typeof o[k]!=='string'||!Number.isFinite(Date.parse(o[k])))bad('Invalid '+k);where.push(`d.publishedAt ${op} ?`);params.push(new Date(o[k]).toISOString());}
 return {limit,offset,where,params};
}
export function createSearch(store,{hybridProvider,relatedProvider}={}) {
 const db=store.db;
 function retrieve(match,o,ids){const f=options(o),where=[...f.where],params=[...f.params];let join='',score='0';if(o.excludeId){where.push('d.id<>?');params.push(o.excludeId);}if(match){join='JOIN search_fts ON search_fts.rowid=d.rowid';where.push('search_fts MATCH ?');params.push(match);score='-bm25(search_fts,8.0,1.0,3.0)';}if(ids){if(!ids.length)return {items:[],total:0,...f};where.push(`d.id IN (${ids.map(()=>'?').join(',')})`);params.push(...ids);}const from=`FROM search_documents d ${join} JOIN records r ON r.kind='items' AND r.id=d.id ${where.length?'WHERE '+where.join(' AND '):''}`;const total=Number(db.prepare(`SELECT count(*) AS n ${from}`).get(...params).n);const rows=db.prepare(`SELECT r.data,${score} AS score ${from} ORDER BY score DESC,d.publishedAt DESC,d.id ASC LIMIT ? OFFSET ?`).all(...params,f.limit,f.offset);return {items:rows.map(r=>({...JSON.parse(r.data),searchScore:r.score,searchMethod:'lexical'})),total,limit:f.limit,offset:f.offset,hasMore:f.offset+rows.length<total};}
 // Fallback only when exact retrieval is empty. Per term: <=192 indexed trigram
 // matches plus <=64 indexed prefix matches, then edit distance <=1 (<=2 for
 // 7+ characters). Adjacent swaps count as one edit; <4-char tokens stay exact.
 function correctionsFor(groups){return groups.map(term=>{if(term.includes(' ')||[...term].length<4||term.length>64)return term;const normalized=term.toLowerCase().normalize('NFD').replace(/\p{M}/gu,'');if(db.prepare('SELECT term FROM search_terms WHERE term=?').get(normalized))return term;const first=[...normalized][0],chars=[...normalized];const grams=[...new Set(chars.slice(0,-2).map((_,i)=>chars.slice(i,i+3).join('')))];const match=grams.map(x=>'"'+x+'"').join(' OR ');
 const trigram=match?db.prepare('SELECT t.term,t.refs AS doc FROM search_term_grams g JOIN search_terms t ON t.rowid=g.rowid WHERE search_term_grams MATCH ? ORDER BY bm25(search_term_grams),t.term LIMIT 192').all(match):[];
 const prefix=db.prepare('SELECT term,refs AS doc FROM search_terms WHERE term>=? AND term<? ORDER BY term LIMIT 64').all(first,first+'\uffff');
 const candidates=[...new Map([...trigram,...prefix].map(x=>[x.term,x])).values()];const max=chars.length>=7?2:1;const choices=candidates.map(x=>({...x,distance:Math.abs([...x.term].length-chars.length)>max?99:editDistance(normalized,x.term)})).filter(x=>x.distance<=max).sort((a,b)=>a.distance-b.distance||b.doc-a.doc||a.term.localeCompare(b.term));return choices[0]?.term||term;});}
 async function search(o={}){const compiled=compileQuery(o.q??'');options(o);if(o.mode&&!['lexical','hybrid'].includes(o.mode))bad('Invalid search mode');let result=retrieve(compiled.match,o),corrections=[];
  if(result.total===0&&compiled.match&&o.fuzzy!==false&&o.fuzzy!=='false'){const corrected=correctionsFor(compiled.groups);corrections=corrected.flatMap((s,i)=>s===compiled.groups[i]?[]:[{from:compiled.groups[i],to:s}]);if(corrections.length)result=retrieve(corrected.map(s=>'"'+s+'"').join(' AND '),o);}
  const base={...result,method:'lexical',query:o.q??'',corrections,fuzzyCandidateLimit:256};
  if(o.mode!=='hybrid')return base;
  if(!hybridProvider)throw Object.assign(new Error('No embedding search provider configured'),{status:503,code:'embedding_search_unavailable'});
  if(o.consent?.confirm!==true||o.consent?.sendQuery!==true||typeof o.consent?.provider!=='string'||!o.consent.provider)bad('Explicit confirm, provider and sendQuery consent required for hybrid search');
  const semantic=await hybridProvider({query:o.q??'',limit:500,consent:o.consent});const p=semantic?.provenance;if(!p||p.provider!==o.consent.provider||typeof p.model!=='string'||!Number.isInteger(p.dimensions)||p.dimensions<1)throw new Error('Invalid embedding search provenance');
  if(!Array.isArray(semantic.candidates)||semantic.candidates.length>500||semantic.candidates.some(c=>typeof c.id!=='string'||!Number.isFinite(c.score)))throw new Error('Invalid embedding candidates');
  // Bounded reciprocal-rank fusion, never mixing incomparable raw vector/BM25 scores.
  const lexical=retrieve(compiled.match,{...o,limit:100,offset:0}).items;const scores=new Map();lexical.forEach((x,i)=>scores.set(x.id,1/(61+i)));[...new Map(semantic.candidates.slice().sort((a,b)=>b.score-a.score||a.id.localeCompare(b.id)).map(c=>[c.id,c])).values()].sort((a,b)=>b.score-a.score||a.id.localeCompare(b.id)).forEach((x,i)=>scores.set(x.id,(scores.get(x.id)||0)+1/(61+i)));
  const ids=[...scores.keys()];const eligible=[];for(let i=0;i<ids.length;i+=100)eligible.push(...retrieve('',{...o,limit:100,offset:0},ids.slice(i,i+100)).items);
  eligible.sort((a,b)=>scores.get(b.id)-scores.get(a.id)||a.id.localeCompare(b.id));const {limit,offset}=options(o);return {...base,items:eligible.slice(offset,offset+limit).map(x=>({...x,searchScore:scores.get(x.id),searchMethod:'hybrid'})),total:eligible.length,hasMore:offset+limit<eligible.length,method:'hybrid',provenance:p,candidateLimited:true};
 }
 async function similar(itemId,o={}){
  const item=store.get('items',itemId);if(!item)throw Object.assign(new Error('Item not found'),{status:404});
  const tokens=[...new Set((item.title+' '+String(item.content||'').slice(0,3000)).toLowerCase().match(/[\p{L}\p{N}]{3,}/gu)||[])].slice(0,32);
  const f=options(o),match=tokens.map(s=>'"'+s+'"').join(' OR ');
  const raw=match?retrieve(match,{...o,excludeId:itemId}):{items:[],total:0,limit:f.limit,offset:f.offset,hasMore:false};
  const base={...raw,method:'lexical',reason:'Shared terms only; not AI or semantic similarity'};
  if(!relatedProvider)return base;
  let vectors;try{vectors=await relatedProvider({itemId,limit:50});}catch(e){if(e.status===409)return base;throw e;}
  if(!vectors?.items?.length)return base;
  const provenance=vectors.provenance;
  if(!provenance||typeof provenance.provider!=='string'||typeof provenance.model!=='string'||!Number.isInteger(provenance.dimensions)||provenance.dimensions<1)throw new Error('Invalid related embedding provenance');
  if(!Array.isArray(vectors.items)||vectors.items.length>50||vectors.items.some(x=>typeof x.item?.id!=='string'||!Number.isFinite(x.score)))throw new Error('Invalid related embedding candidates');
  const lexical=match?retrieve(match,{...o,excludeId:itemId,limit:100,offset:0}).items:[];
  const scores=new Map(lexical.map((x,i)=>[x.id,1/(61+i)])),seen=new Set();
  vectors.items.slice().sort((a,b)=>b.score-a.score||a.item.id.localeCompare(b.item.id)).forEach((x,i)=>{if(x.item.id===itemId||seen.has(x.item.id))return;seen.add(x.item.id);scores.set(x.item.id,(scores.get(x.item.id)||0)+1/(61+i));});
  const ids=[...scores.keys()],eligible=[];
  for(let i=0;i<ids.length;i+=100)eligible.push(...retrieve('',{...o,excludeId:itemId,limit:100,offset:0},ids.slice(i,i+100)).items);
  eligible.sort((a,b)=>scores.get(b.id)-scores.get(a.id)||a.id.localeCompare(b.id));
  if(!eligible.some(x=>seen.has(x.id)))return base;
  return {...base,items:eligible.slice(f.offset,f.offset+f.limit).map(x=>({...x,searchScore:scores.get(x.id),searchMethod:'hybrid-related'})),total:eligible.length,hasMore:f.offset+f.limit<eligible.length,method:'hybrid-related',provenance,candidateLimited:true,reason:'Reciprocal-rank fusion of shared terms and compatible stored embeddings; no remote request'};
 }

 return {search,similar};
}

function initializeDictionary(db) {
 db.function('search_tokens',{deterministic:true},text=>JSON.stringify([...new Set(String(text||'').toLowerCase().normalize('NFD').replace(/\p{M}/gu,'').match(/[\p{L}\p{N}]+/gu)||[])].filter(t=>t.length>=3&&t.length<=64)));
 db.exec([
 "CREATE TABLE IF NOT EXISTS search_terms(term TEXT PRIMARY KEY, refs INTEGER NOT NULL);",
 "CREATE TABLE IF NOT EXISTS search_document_terms(documentId TEXT NOT NULL,term TEXT NOT NULL,PRIMARY KEY(documentId,term));",
 "CREATE VIRTUAL TABLE IF NOT EXISTS search_term_grams USING fts5(term,content='search_terms',content_rowid='rowid',tokenize='trigram');",
 "CREATE TRIGGER IF NOT EXISTS search_term_add AFTER INSERT ON search_terms BEGIN INSERT INTO search_term_grams(rowid,term) VALUES(new.rowid,new.term); END;",
 "CREATE TRIGGER IF NOT EXISTS search_term_remove AFTER DELETE ON search_terms BEGIN INSERT INTO search_term_grams(search_term_grams,rowid,term) VALUES('delete',old.rowid,old.term); END;",
 "CREATE TRIGGER IF NOT EXISTS search_reference_add AFTER INSERT ON search_document_terms BEGIN INSERT INTO search_terms(term,refs) VALUES(new.term,1) ON CONFLICT(term) DO UPDATE SET refs=refs+1; END;",
 "CREATE TRIGGER IF NOT EXISTS search_reference_remove AFTER DELETE ON search_document_terms BEGIN UPDATE search_terms SET refs=refs-1 WHERE term=old.term; DELETE FROM search_terms WHERE term=old.term AND refs<=0; END;",
 "CREATE TRIGGER IF NOT EXISTS search_dictionary_add AFTER INSERT ON search_documents BEGIN INSERT OR IGNORE INTO search_document_terms SELECT new.id,value FROM json_each(search_tokens(new.title||' '||new.content||' '||new.author)); END;",
 "CREATE TRIGGER IF NOT EXISTS search_dictionary_update AFTER UPDATE ON search_documents BEGIN DELETE FROM search_document_terms WHERE documentId=old.id; INSERT OR IGNORE INTO search_document_terms SELECT new.id,value FROM json_each(search_tokens(new.title||' '||new.content||' '||new.author)); END;",
 "CREATE TRIGGER IF NOT EXISTS search_dictionary_remove AFTER DELETE ON search_documents BEGIN DELETE FROM search_document_terms WHERE documentId=old.id; END;"
 ].join('\n'));
 if(!db.prepare('SELECT 1 FROM search_migrations WHERE version=2').get()){
  db.exec('BEGIN IMMEDIATE');try{db.exec("INSERT OR IGNORE INTO search_document_terms SELECT d.id,j.value FROM search_documents d,json_each(search_tokens(d.title||' '||d.content||' '||d.author)) j; INSERT INTO search_migrations VALUES(2);");db.exec('COMMIT');}catch(e){db.exec('ROLLBACK');throw e;}
 }
}
