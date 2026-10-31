import {searchOptions} from '../pages/search.js';
export function captureSearch(state,form){const d=new FormData(form);state.query=String(d.get('q')||'');state.searchFilters=Object.fromEntries(['sourceId','protocol','read','saved','after','before'].map(k=>[k,String(d.get(k)||'')]));state.searchFilters.fuzzy=d.has('fuzzy');}
export function providerApproval(status,scope){return `Approve one semantic search?\nProvider: ${status.provider}\nDestination: ${status.endpoint||status.destination||status.provider}\nQuery: ${scope}\n\nOnly this query is sent for embedding, not story text. Provider charges and retention terms may apply, including failed or canceled requests. This does not authorize later queries. Proceed?`;}
export function bindSearch({state,api,render,toast,$,refreshResearch}){
 document.querySelectorAll('[data-find-related]').forEach(b=>b.onclick=async()=>{const id=b.dataset.findRelated,epoch=state.navigationEpoch;b.disabled=true;try{const result=await api('/items/'+encodeURIComponent(id)+'/similar?limit=5');if(epoch===state.navigationEpoch){state.relatedResults||={};state.relatedResults[id]=result;state.extraItems||={};for(const item of result.items||[])state.extraItems[item.id]=item;render();document.querySelectorAll('.library-related').forEach(d=>d.open=true);}}catch(error){toast(error.message);b.disabled=false;}});
 const run=async(mode='lexical',offset=0)=>{
  if(state.searchLoading)return;const form=$('#search-form');if(form)captureSearch(state,form);
  if(state.searchFilters.before&&state.searchFilters.after>state.searchFilters.before){toast('The start date must be before the end date.');return;}
  const options=searchOptions(state,offset);
  if(mode==='hybrid'){
   const m=state.modelStatus;if(!m?.enabled||!m.models?.embedding||!m.provider){toast('An embedding provider is not available.');return;}
   if(!state.query.trim()){toast('Enter a query to search by meaning.');return;}
   if(!confirm(providerApproval(m,state.query)))return;
   options.mode='hybrid';options.consent={provider:m.provider,sendQuery:true,confirm:true};
  }
  const request=Symbol('search'),epoch=state.navigationEpoch;state.searchRequest=request;state.searchLoading=true;state.searchError='';render();
  try{const result=mode==='hybrid'?await api('/search',{method:'POST',body:options}):await api('/search?'+new URLSearchParams(options));if(state.searchRequest===request&&epoch===state.navigationEpoch){state.searchResults=result;state.items=result.items||[];state.searchOffset=offset;}}
  catch(error){if(epoch===state.navigationEpoch)state.searchError=error.message;}
  finally{if(state.searchRequest===request){state.searchLoading=false;if(epoch===state.navigationEpoch)render();}}
 };
 $('#search-form')?.addEventListener('submit',e=>{e.preventDefault();run();});
 $('#search-form')?.addEventListener('input',e=>captureSearch(state,e.currentTarget));
 $('#search-form')?.addEventListener('change',e=>captureSearch(state,e.currentTarget));
 $('[data-action="hybrid-search"]')?.addEventListener('click',()=>run('hybrid'));
 document.querySelectorAll('[data-search-page]').forEach(b=>b.onclick=()=>run(state.searchResults?.method==='hybrid'?'hybrid':'lexical',Number(b.dataset.searchPage)));
 $('#research-question')?.addEventListener('input',e=>state.researchQuestion=e.target.value);
 document.querySelectorAll('[name="sourceIds"]').forEach(b=>b.onchange=()=>state.researchSourceIds=[...document.querySelectorAll('[name="sourceIds"]:checked')].map(x=>x.value));
 $('#research-form')?.addEventListener('submit',async event=>{
  event.preventDefault();if(state.busy)return;const d=new FormData(event.target),query=String(d.get('query')||'').trim(),sourceIds=d.getAll('sourceIds'),m=state.modelStatus;
  if(!query||!sourceIds.length||sourceIds.length>20){toast('Ask a question and select between 1 and 20 sources.');return;}
  if(!m?.enabled||!m.models?.summary||!m.provider){toast('A summary provider is not available.');return;}
  const sources=sourceIds.map(id=>state.sources.find(s=>s.id===id)),privateSources=sources.filter(s=>!s.share);
  if(!confirm(`Approve one library research job?\n\nProvider: ${m.provider}\nDestination: ${m.endpoint||m.destination||m.provider}\nQuestion: ${query}\nSources: ${sources.map(s=>s.title).join(', ')}\n\nThe agent may search and read only these stored sources, with up to four model requests. Your question and retrieved titles/content will be sent to this provider. ${privateSources.length?'LOCAL-ONLY CONTENT: '+privateSources.map(s=>s.title).join(', ')+'. Approving explicitly authorizes sending their content and your private reading interests.':''}\n\nProvider charges may apply, including failed or canceled requests. Cancellation cannot undo disclosure or guarantee no charges. No live web search, subscriptions, or publication. Proceed?`))return;
  const epoch=state.navigationEpoch;state.busy=true;render();
  try{await api('/agents/search',{method:'POST',body:{query,sourceIds,confirm:true,consent:{provider:m.provider,sendQuery:true,sendContent:true,includePrivate:privateSources.length>0}}});if(epoch===state.navigationEpoch)await refreshResearch();toast('Library research submitted.');}catch(error){toast(error.message);}finally{state.busy=false;if(epoch===state.navigationEpoch)render();}
 });
 $('[data-action="refresh-research"]')?.addEventListener('click',refreshResearch);
 document.querySelectorAll('[data-cancel-research]').forEach(b=>b.onclick=async()=>{b.disabled=true;try{await api('/agents/search/'+encodeURIComponent(b.dataset.cancelResearch)+'/cancel',{method:'POST',body:{}});await refreshResearch();toast('Cancellation requested. Requests already sent may still be billed.');}catch(error){toast(error.message);b.disabled=false;}});
}
