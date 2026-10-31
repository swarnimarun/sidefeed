export const state = {
 view:'inbox', items:[], sources:[], drafts:[], runs:[], fleet:{}, status:null,
 query:'', filter:'all', sourceFilter:'', selected:null, loading:true,error:'',busy:false,
 showConnection:false,menuOpen:false, draftTitle:'',draftContent:'',editDraft:null,draftDirty:false,
 modelStatus:{},modelJobs:[],modelArtifacts:[],similar:null,thread:null,threadError:'',threadLoading:false
};
export function visibleItems(s){return s.items.filter(i=>(s.view!=='saved'||i.saved)&&(s.filter!=='unread'||!i.read)&&(s.filter!=='read'||i.read)&&(!s.sourceFilter||i.sourceId===s.sourceFilter)&&(!s.query||`${i.title} ${i.content} ${i.author}`.toLowerCase().includes(s.query.toLowerCase())));}
