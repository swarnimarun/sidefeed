// No provider HTML is trusted. The caller supplies the same escaping and URL guards as the reader.
export function threadPanel(snapshot,{escape,safeUrl,date}) {
 if(!snapshot)return '';
 const nodes=Array.isArray(snapshot.nodes)?snapshot.nodes.slice(0,100):[], byId=new Map(nodes.map(n=>[n.uri,n]));
 const children=new Map(),hasParent=new Set();
 for(const edge of (snapshot.edges||[]).slice(0,200)){
  if(edge.type!=='reply'||!byId.has(edge.from)||!byId.has(edge.to)||edge.from===edge.to)continue;
  if(!children.has(edge.to))children.set(edge.to,[]);
  if(!children.get(edge.to).includes(edge.from))children.get(edge.to).push(edge.from);
  hasParent.add(edge.from);
 }
 const seen=new Set();
 function node(uri,depth=0){
  if(seen.has(uri))return '';
  if(depth>8)return '<div class="thread-placeholder">More depth exists in this snapshot; use the original source to inspect it.</div>';
  seen.add(uri);const n=byId.get(uri);if(!n)return '';
  const body=n.state==='post'?`<article class="thread-node"><div class="article-meta"><strong>${escape(n.author||n.authorDid||'Unknown author')}</strong><span>· ${date(n.publishedAt)}</span></div><p>${escape(n.content||n.title||'No text supplied')}</p>${safeUrl(n.url)!=='#'?`<a class="button small" target="_blank" rel="noopener noreferrer" href="${safeUrl(n.url)}">Open original ↗</a>`:''}<details><summary class="fineprint">Provenance</summary><pre class="code">${escape(JSON.stringify({uri:n.uri,cid:n.cid,authorDid:n.authorDid,reply:n.reply},null,2))}</pre></details></article>`:`<div class="thread-placeholder"><strong>${escape(n.state==='blocked'?'Blocked post':n.state==='not-found'?'Post not found':'Parent or post unavailable')}</strong><p class="fineprint">${escape(uri)}</p><span>Content was not supplied by this public snapshot. No missing text has been inferred.</span></div>`;
  const replies=(children.get(uri)||[]).map(c=>node(c,depth+1)).join('');
  return body+(replies?`<div class="thread-children">${replies}</div>`:'');
 }
 const roots=nodes.filter(n=>!hasParent.has(n.uri));
 let tree=roots.map(n=>node(n.uri)).join('');
 // Keep disconnected/partially truncated nodes visible; seen prevents duplicate/cyclic rendering.
 tree+=nodes.filter(n=>!seen.has(n.uri)).map(n=>node(n.uri)).join('');
 return `<section class="card thread-panel"><div class="section-title"><h2>AT Protocol thread snapshot</h2><span class="pill pending">Partial snapshot</span></div><div class="notice">A bounded public AppView response, not a complete or live conversation. Blocked, deleted, missing, or truncated posts may not be available.${snapshot.truncated?' This snapshot was truncated.':''}</div><p class="fineprint">Snapshot reasons: ${escape((snapshot.reasons||['bounded-appview-snapshot']).join(', '))}</p>${tree||'<p class="muted">No thread nodes were returned.</p>'}</section>`;
}
