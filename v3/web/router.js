export const views=['inbox','saved','search','threads','sources','compose','fleet','agents','models','settings'];
export const route=()=>views.includes(location.hash.slice(1))?location.hash.slice(1):'inbox';
export function createRouter(state,load){
 function navigate(view){if(!views.includes(view))view='inbox';if(state.view!==view){state.query='';state.filter='all';state.sourceFilter='';}state.view=view;state.selected=null;state.menuOpen=false;state.navigationEpoch=(state.navigationEpoch||0)+1;state.threadLoading=false;if(location.hash!=='#'+view)history.pushState(null,'','#'+view);load();}
 window.addEventListener('popstate',()=>navigate(route()));
 window.addEventListener('hashchange',()=>{if(route()!==state.view)navigate(route());});
 return navigate;
}
