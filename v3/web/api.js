let token='';
export const connection={set(value){token=value;},get active(){return Boolean(token);}};
export async function api(path,options={}){
 const response=await fetch('/api/v3'+path,{...options,headers:{'Content-Type':'application/json',...(token?{Authorization:'Bearer '+token}:{}),...options.headers},body:options.body?JSON.stringify(options.body):undefined});
 let data;try{data=await response.json();}catch{throw new Error(`The node returned ${response.status}. Check the Sidefeed service and try again.`);}
 if(!response.ok)throw new Error(data.error?.message||data.error||`Request failed (${response.status})`);
 return data;
}
