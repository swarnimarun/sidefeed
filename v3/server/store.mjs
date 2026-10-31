import {DatabaseSync} from 'node:sqlite';
import {randomUUID} from 'node:crypto';
import {initializeSearch} from './search.mjs';
export class Store {
 constructor(path=':memory:'){this.db=new DatabaseSync(path);this.db.exec('PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS records (kind TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(kind,id)); CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT, sourceId TEXT NOT NULL, data TEXT NOT NULL);'); if(Number(this.db.prepare('PRAGMA user_version').get().user_version)<1)this.db.exec('PRAGMA user_version=1'); initializeSearch(this.db);}
 all(k){return this.db.prepare('SELECT data FROM records WHERE kind=?').all(k).map(r=>JSON.parse(r.data));}
 get(k,id){const r=this.db.prepare('SELECT data FROM records WHERE kind=? AND id=?').get(k,id);return r?JSON.parse(r.data):null;}
 put(k,v){this.db.prepare('INSERT INTO records VALUES(?,?,?) ON CONFLICT(kind,id) DO UPDATE SET data=excluded.data').run(k,v.id,JSON.stringify(v));return v;}
 create(k,v){return this.put(k,{id:randomUUID(),createdAt:new Date().toISOString(),...v});}
 delete(k,id){this.db.prepare('DELETE FROM records WHERE kind=? AND id=?').run(k,id);}
 transaction(fn){this.db.exec('BEGIN IMMEDIATE');try{const x=fn();this.db.exec('COMMIT');return x;}catch(e){this.db.exec('ROLLBACK');throw e;}}
 event(sourceId,data){const r=this.db.prepare('INSERT INTO events(sourceId,data) VALUES(?,?)').run(sourceId,JSON.stringify(data));return Number(r.lastInsertRowid);}
 events(cursor=0){return this.db.prepare('SELECT seq, sourceId, data FROM events WHERE seq>? ORDER BY seq LIMIT 1000').all(cursor).map(r=>({...JSON.parse(r.data),seq:r.seq,sourceId:r.sourceId}));}
 close(){this.db.close();}
}
