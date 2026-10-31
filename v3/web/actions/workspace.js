// Page-local behavior. All writes are explicit user actions; provider consent is per job.
export function bindWorkspace({state,api,mutate,render,toast,navigate,$}) {
 const all=(selector,fn)=>document.querySelectorAll(selector).forEach(fn);
 const submit=(id,fn)=>$(id)?.addEventListener('submit',event=>{event.preventDefault();if(state.busy)return;fn(new FormData(event.target),event);});
 submit('#source-form',data=>mutate('/sources',Object.fromEntries(data),'Subscribed. Use Fetch now to collect stories.'));
 submit('#discovery-form',async data=>{await mutate('/agents/runs',{url:data.get('url'),goal:'Discover public feeds advertised by this website'},'Discovery finished. Review its result before approving.');});
 all('[data-refresh]',b=>b.onclick=()=>mutate('/sources/'+encodeURIComponent(b.dataset.refresh)+'/refresh',{},'Feed fetch completed.'));
 all('[data-pause]',b=>b.onclick=()=>{const s=state.sources.find(s=>s.id===b.dataset.pause);if(s)mutate('/sources/'+encodeURIComponent(s.id),{paused:!s.paused},s.paused?'Source resumed':'Source paused','PATCH');});
 all('[data-remove-source]',b=>b.onclick=()=>{if(confirm('Remove this source? This cannot be undone from this screen.'))mutate('/sources/'+encodeURIComponent(b.dataset.removeSource),null,'Source removed','DELETE');});
 all('[data-approve]',b=>b.onclick=()=>{if(confirm('Approve this discovery proposal and subscribe to its proposed feeds?'))mutate('/agents/runs/'+encodeURIComponent(b.dataset.approve)+'/approve',{confirm:true},'Proposal approved. Check your sources.');});
 all('[data-deny]',b=>b.onclick=()=>mutate('/agents/runs/'+encodeURIComponent(b.dataset.deny)+'/deny',{},'Proposal denied.'));
 submit('#draft-form',data=>mutate(state.editDraft?'/drafts/'+encodeURIComponent(state.editDraft):'/drafts',Object.fromEntries(data),'Private draft saved.',state.editDraft?'PATCH':'POST'));
 all('[data-edit-draft]',b=>b.onclick=()=>{const d=state.drafts.find(d=>d.id===b.dataset.editDraft);if(!d)return;if((state.draftTitle||state.draftContent)&&!confirm('Replace the current editor text with this saved draft?'))return;state.editDraft=d.id;state.draftDirty=false;state.draftTitle=d.title;state.draftContent=d.content;render();$('#draft-title')?.focus();});
 $('[data-action="cancel-edit"]')?.addEventListener('click',()=>{if(!(state.draftTitle||state.draftContent)||confirm('Discard unsaved changes in this editor? The saved draft stays unchanged.')){state.editDraft=null;state.draftTitle='';state.draftContent='';state.draftDirty=false;render();}});
 $('#draft-title')?.addEventListener('input',event=>{state.draftDirty=true;state.draftTitle=event.target.value;const p=$('#preview-title');if(p)p.textContent=event.target.value||'Your title';});
 $('#draft-content')?.addEventListener('input',event=>{state.draftDirty=true;state.draftContent=event.target.value;const p=$('#preview-content');if(p)p.textContent=event.target.value||'Your words will appear here as you write.';});
 all('[data-publish]',b=>b.onclick=()=>{if(confirm('Publish this draft on this local node? It will no longer be private. This does not send it to ActivityPub or AT Protocol.'))mutate('/drafts/'+encodeURIComponent(b.dataset.publish)+'/publish',{confirm:true},'Published locally. No remote protocol delivery was requested.');});
 $('#thread-uri')?.addEventListener('input',event=>{state.threadUri=event.target.value;});
 submit('#thread-form',async data=>{
  if(state.threadLoading)return;const uri=String(data.get('uri')).trim();state.threadUri=uri;
  if(!/^at:\/\/did:[^/]+\/app\.bsky\.feed\.post\/[^/?#]+$/.test(uri)){toast('Use a canonical DID-based at://…/app.bsky.feed.post/… URI.');return;}
  const request=Symbol('thread'),epoch=state.navigationEpoch;state.threadRequest=request;state.threadLoading=true;state.threadError='';state.thread=null;render();
  const current=()=>state.threadRequest===request&&state.navigationEpoch===epoch;
  try{const result=await api('/threads?'+new URLSearchParams({uri}));if(current())state.thread=result;}catch(error){if(current())state.threadError=error.message;}finally{if(current()){state.threadLoading=false;render();}}
 });
 submit('#model-job-form',async data=>{
  if(!state.modelStatus.enabled)return;const itemIds=data.getAll('itemIds'),operation=data.get('operation');
  if(!itemIds.length){toast('Select at least one story.');return;}
  const max=state.modelStatus.limits?.maxItems;if(max&&itemIds.length>max){toast(`Select at most ${max} stories for one job.`);return;}
  if(!state.modelStatus.models?.[operation==='embed'?'embedding':'classification']){toast('No model is configured for this operation.');return;}
  const provider=state.modelStatus.provider;if(typeof provider!=='string'||!provider){toast('The server has not reported a provider identity.');return;}
  const titles=itemIds.map(id=>state.items.find(i=>i.id===id)?.title||id),privateIds=itemIds.filter(id=>{const item=state.items.find(i=>i.id===id);return !state.sources.find(s=>s.id===item?.sourceId)?.share;});
  const destination=state.modelStatus.endpoint||state.modelStatus.destination||provider;
  if(!confirm(`Approve one ${operation} job?\n\nProvider: ${provider}\nDestination: ${destination}\nScope: ${itemIds.length} selected stories\n${titles.map(t=>'• '+t).join('\n')}\n\nTheir titles and content will be sent to this configured provider. ${privateIds.length?`This includes ${privateIds.length} stories from local-only/private-interest sources. By approving, you explicitly authorize sending that private content to this provider.`:'No local-only source items are selected.'} Provider charges and server limits apply, including for failed or canceled requests. There is no currency-cost guarantee. This approval does not authorize future jobs. Proceed?`))return;
  await mutate('/models/jobs',{operation,itemIds,confirm:true,consent:{sendContent:true,provider,includePrivate:privateIds.length>0}},'Model job submitted. Refresh to inspect its result.');
 });
 all('[data-cancel-model]',b=>b.onclick=()=>mutate('/models/jobs/'+encodeURIComponent(b.dataset.cancelModel)+'/cancel',{},'Cancellation requested. Inspect the resulting job state.'));
 submit('#similar-form',async data=>{state.busy=true;const epoch=state.navigationEpoch;try{const result=await api('/models/similar',{method:'POST',body:{itemId:data.get('itemId'),limit:10}});if(epoch===state.navigationEpoch)state.similar=result;}catch(error){toast(error.message);}finally{state.busy=false;render();}});
}
