export function bindSubscriptions({state,api,mutate,render,toast,$}){
 const reset=()=>{state.subscriptionRequest=null;state.subscriptionLoading=false;state.subscriptionPreview=null;state.subscriptionError='';state.subscriptionFingerprint=null;};
 const fingerprint=()=>JSON.stringify([state.subscriptionKind||'rss',(state.subscriptionInput||'').trim()]);
 $('#subscription-input')?.addEventListener('input',event=>{
  const cursor=event.target.selectionStart;state.subscriptionInput=event.target.value;reset();render();const input=$('#subscription-input');input?.focus();if(input&&cursor!==null)input.setSelectionRange(cursor,cursor);
 });
 $('#subscription-kind')?.addEventListener('change',event=>{state.subscriptionKind=event.target.value;reset();render();$('#subscription-kind')?.focus();});
 $('[data-subscription-cancel]')?.addEventListener('click',()=>{if(state.busy)return;reset();render();$('#subscription-input')?.focus();});
 $('#subscription-form')?.addEventListener('submit',async event=>{
  event.preventDefault();if(state.busy||state.subscriptionLoading)return;
  const form=new FormData(event.target),input=String(form.get('input')||'').trim(),kind=String(form.get('kind')||'rss');
  if(!input||!['rss','atproto','activitypub'].includes(kind))return;
  reset();state.subscriptionInput=input;state.subscriptionKind=kind;
  const request=Symbol('subscription-preview'),epoch=state.navigationEpoch,key=fingerprint();
  state.subscriptionRequest=request;state.subscriptionLoading=true;render();
  const current=()=>state.subscriptionRequest===request&&state.navigationEpoch===epoch&&fingerprint()===key;
  try{
   const result=await api('/subscriptions/preview',{method:'POST',body:{input,kind}});
   if(current()){
    if(!result?.id||!result?.source)throw new Error('The source preview was incomplete. Nothing was subscribed.');
    state.subscriptionPreview=result;state.subscriptionFingerprint=key;
   }
  }catch(error){if(current())state.subscriptionError=error.message||'Could not preview this source. Nothing was subscribed.';}
  finally{if(state.subscriptionRequest===request){state.subscriptionLoading=false;if(current()){render();$('#subscription-preview')?.scrollIntoView?.({block:'nearest'});}}}
 });
 $('[data-subscription-confirm]')?.addEventListener('click',async()=>{
  if(state.busy||state.subscriptionLoading)return;const preview=state.subscriptionPreview;
  if(!preview?.id||state.subscriptionFingerprint!==fingerprint()){toast('Preview this source again before subscribing.');return;}
  if(!confirm(`Subscribe locally to ${preview.source.title||preview.source.url||'this public source'}?\n\n${preview.source.url||''}\n\nThis authorizes this node to poll public content and add it to your reading room. It does not send a remote Follow, authenticate as you, or publish anything.`))return;
  const epoch=state.navigationEpoch;state.error='';
  await mutate('/subscriptions',{previewId:preview.id,confirm:true},'Subscribed locally. No remote Follow or publication was sent.');
  if(!state.error&&state.subscriptionPreview===preview){reset();if(state.navigationEpoch===epoch)render();}
 });
}
