import {searchPage,searchOptions} from './pages/search.js';
import {bindSearch} from './actions/search.js';
import {bindSubscriptions} from './actions/subscriptions.js';
import {state} from './state.js';
import {api,connection} from './api.js';
import {createRouter,route} from './router.js';
import * as html from './components/html.js';
import {shell} from './components/shell.js';
import {reader,detail} from './pages/reader.js';
import {workspace} from './pages/workspace.js';
import {bindWorkspace} from './actions/workspace.js';
const $=(selector,root=document)=>root.querySelector(selector);
let loadSequence=0,toastTimer,researchTimer;
const navigate=createRouter(state,load);
function toast(message){$('#toast').textContent=message;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').textContent='',5000);}
function connectionForm(){return `<section class="connection-panel"><h2>Connect to your node</h2><p>Your owner token stays in this page’s memory only. Reloading clears it. Use HTTPS for a remote node.</p><form id="connection-form" class="inline-form"><div class="field"><label for="owner-token">Owner token</label><input id="owner-token" name="token" type="password" autocomplete="off" required></div><button class="button primary">Connect</button>${connection.active?'<button class="button" type="button" data-action="disconnect">Disconnect</button>':''}<button class="button" type="button" data-action="close-connection">Close</button></form></section>`;}
function render(){
 const focused=document.activeElement,focusId=focused?.id,selection=typeof focused?.selectionStart==='number'?[focused.selectionStart,focused.selectionEnd]:null,filtersOpen=$('#search-filters')?.open;
 const reading=['inbox','saved','search'].includes(state.view);
 const content=state.loading?'<div class="loading" role="status">Opening your reading room…</div>':reading?(state.selected?detail(state):state.view==='search'?searchPage(state):reader(state)): `<div class="workspace-page">${workspace(state,html)}</div>`;
 $('#app').innerHTML=shell(state,`${state.showConnection?connectionForm():''}${state.error?`<div class="notice error" role="alert">${html.escape(state.error)} <button class="button" data-action="reload">Try again</button></div>`:''}${content}`,connection.active);
 bind();
 if(filtersOpen&&$('#search-filters'))$('#search-filters').open=true;
 if(focusId){const input=document.getElementById(focusId);input?.focus({preventScroll:true});if(selection&&input?.setSelectionRange)try{input.setSelectionRange(...selection);}catch{}}
}
async function load(){const seq=++loadSequence;state.loading=true;state.error='';render();try{
 const query=new URLSearchParams({limit:'1000'});if(state.view==='saved')query.set('saved','true');if(state.filter==='unread')query.set('unread','true');if(state.filter==='read')query.set('read','true');if(state.sourceFilter)query.set('sourceId',state.sourceFilter);if(state.query)query.set('q',state.query);
 const [status,sources,items]=await Promise.all([api('/status'),api('/sources'),state.view==='search'?api('/search?'+new URLSearchParams(searchOptions(state))):api('/items?'+query)]);
 if(seq!==loadSequence)return;state.status=status;state.sources=sources.sources||[];state.items=items.items||[];if(state.view==='search')state.searchResults=items;
 const view=state.view;
 if(view==='search'){const d=await Promise.all([api('/models/status'),api('/agents/search')]);if(seq===loadSequence){state.modelStatus=d[0];state.agentSearchJobs=d[1].jobs||[];scheduleResearch();}}
 if(view==='compose'){const d=await api('/drafts');if(seq===loadSequence)state.drafts=d.drafts||[];}
 if(['sources','agents'].includes(view)){const d=await api('/agents/runs');if(seq===loadSequence)state.runs=d.runs||[];}
 if(view==='fleet'){const d=await api('/fleet');if(seq===loadSequence)state.fleet=d;}
 if(view==='models'){const d=await Promise.all([api('/models/status'),api('/models/jobs'),api('/models/artifacts')]);if(seq===loadSequence){state.modelStatus=d[0];state.modelJobs=d[1].jobs||[];state.modelArtifacts=d[2].artifacts||[];}}
 }catch(error){if(seq===loadSequence){state.error=error.message;state.status=null;state.showConnection=true;}}finally{if(seq===loadSequence){state.loading=false;render();}}}
async function mutate(path,body,message,method='POST'){
 if(state.busy)return;state.busy=true;document.querySelectorAll('button').forEach(b=>b.disabled=true);
 try{const response=await api(path,{method,body});if(method==='PATCH'&&path.startsWith('/items/')){state.extraItems||={};state.extraItems[response.id]=response;}if(path==='/drafts'||(method==='PATCH'&&path.startsWith('/drafts/'))){state.draftTitle='';state.draftContent='';state.editDraft=null;state.draftDirty=false;}toast(message);await load();}
 catch(error){state.error=error.message;toast(error.message);}finally{state.busy=false;render();}
}
function skipToContent(event){event.preventDefault();const main=$('#main');main?.focus();main?.scrollIntoView({block:'start'});}
function bind(){
 $('.skip')?.addEventListener('click',skipToContent);
 document.querySelectorAll('a[href^="#"]:not(.skip)').forEach(a=>a.onclick=event=>{if(event.ctrlKey||event.metaKey||event.shiftKey||event.altKey)return;event.preventDefault();navigate(a.hash.slice(1));});
 document.querySelectorAll('[data-nav]').forEach(b=>b.onclick=()=>navigate(b.dataset.nav));
 $('[data-action="menu"]')?.addEventListener('click',()=>{state.menuOpen=!state.menuOpen;render();$('[data-action="menu"]').focus();});
 document.querySelectorAll('[data-archive]').forEach(b=>b.onclick=()=>{state.navigationEpoch=(state.navigationEpoch||0)+1;state.threadLoading=false;state.view='inbox';state.filter=b.dataset.archive;state.sourceFilter='';state.query='';state.selected=null;state.menuOpen=false;history.pushState(null,'','#inbox');load();});
 document.querySelectorAll('[data-source-filter]').forEach(b=>b.onclick=()=>{state.navigationEpoch=(state.navigationEpoch||0)+1;state.threadLoading=false;state.view='inbox';state.sourceFilter=b.dataset.sourceFilter;state.filter='all';state.query='';state.selected=null;state.menuOpen=false;history.pushState(null,'','#inbox');load();});
 document.querySelectorAll('[data-filter]').forEach(b=>b.onclick=()=>{state.filter=b.dataset.filter;load();});
 document.querySelectorAll('[data-open]').forEach(b=>b.onclick=()=>{state.selected=b.dataset.open;render();$('#main').focus();window.scrollTo(0,0);});
 document.querySelectorAll('[data-save]').forEach(b=>b.onclick=()=>{const i=state.items.find(i=>i.id===b.dataset.save)||state.extraItems?.[b.dataset.save];mutate('/items/'+encodeURIComponent(i.id),{saved:!i.saved},i.saved?'Removed from saved':'Saved for later','PATCH');});
 document.querySelectorAll('[data-read]').forEach(b=>b.onclick=()=>{const i=state.items.find(i=>i.id===b.dataset.read)||state.extraItems?.[b.dataset.read];mutate('/items/'+encodeURIComponent(i.id),{read:!i.read},i.read?'Marked unread':'Marked read','PATCH');});
 $('[data-action="back"]')?.addEventListener('click',()=>{state.selected=null;render();});
 document.querySelectorAll('[data-action="reload"]').forEach(b=>b.onclick=load);
 document.querySelectorAll('[data-action="connect"]').forEach(b=>b.addEventListener('click',()=>{state.showConnection=true;render();$('#owner-token')?.focus();}));
 $('[data-action="close-connection"]')?.addEventListener('click',()=>{state.showConnection=false;render();});
 $('#connection-form')?.addEventListener('submit',event=>{event.preventDefault();connection.set(new FormData(event.target).get('token').trim());state.showConnection=false;load();});
 $('[data-action="disconnect"]')?.addEventListener('click',()=>{connection.set('');state.navigationEpoch=(state.navigationEpoch||0)+1;Object.assign(state,{searchResults:null,agentSearchJobs:[],researchQuestion:'',researchSourceIds:[],relatedResults:{},extraItems:{},subscriptionPreview:null,subscriptionRequest:null,items:[],sources:[],drafts:[],runs:[],fleet:{},modelStatus:{},modelJobs:[],modelArtifacts:[],similar:null,thread:null,selected:null,draftTitle:'',draftContent:'',draftDirty:false,editDraft:null,showConnection:true});load();});

 bindWorkspace({state,api,mutate,render,toast,navigate,$});
 bindSearch({state,api,render,toast,$,refreshResearch});
 bindSubscriptions({state,api,mutate,render,toast,$});
}

function scheduleResearch(){clearTimeout(researchTimer);if(state.view==='search'&&state.agentSearchJobs.some(j=>['queued','running'].includes(j.status)))researchTimer=setTimeout(refreshResearch,1800);}
async function refreshResearch(){const epoch=state.navigationEpoch;try{const data=await api('/agents/search');if(epoch===state.navigationEpoch&&state.view==='search'){state.agentSearchJobs=data.jobs||[];render();scheduleResearch();}}catch(error){toast(error.message);}}
window.addEventListener('keydown',event=>{if(event.key==='Escape'&&state.menuOpen){state.menuOpen=false;render();$('.menu-toggle')?.focus();}});
window.addEventListener('beforeunload',event=>{if(state.draftDirty){event.preventDefault();event.returnValue='';}});
state.view=route();load();
