export const escape=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export const safeUrl=value=>{try{const u=new URL(value);return ['http:','https:'].includes(u.protocol)?escape(u.href):'#';}catch{return '#';}};
export const date=value=>{if(!value)return 'Not yet fetched';const d=new Date(value);return Number.isNaN(d.getTime())?'Unknown date':d.toLocaleDateString(undefined,{month:'short',day:'numeric'});};
export const pill=(text,pending=false)=>`<span class="pill ${pending?'pending':''}">${escape(text)}</span>`;
export const heading=(kicker,title,description,action='')=>`<header class="page-heading"><div class="eyebrow">${escape(kicker)}</div><h1>${escape(title)}</h1><p>${escape(description)}</p>${action}</header>`;
export const empty=(title,text)=>`<section class="empty"><h2>${escape(title)}</h2><p>${escape(text)}</p></section>`;
