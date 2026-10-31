// Hash events may arrive after a click and after the user starts typing.
// Applying an already-active route must never reset that user's newer state.
export function transitionView(state, view) {
 if(state.view===view)return false;
 state.view=view;state.selected=null;state.thread=null;state.threadError='';state.threadLoading=false;state.query='';state.filter='all';
 return true;
}
