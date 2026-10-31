// Relatedness is transparent local metadata, never a fabricated AI recommendation.
export function relatedEntries(item, items, limit=3) {
 if(!item)return [];
 return items.filter(other=>other.id!==item.id).map(other=>{
  const shared=(item.tags||[]).filter(tag=>(other.tags||[]).includes(tag));
  const sameSource=!!item.sourceId&&item.sourceId===other.sourceId;
  return {item:other,reason:shared.length?'Shared keyword: '+shared.join(', '):sameSource?'Same source':'',rank:shared.length*2+(sameSource?1:0)};
 }).filter(entry=>entry.rank>0).sort((a,b)=>b.rank-a.rank).slice(0,limit);
}
