//! Replaceable embedding providers and hybrid semantic retrieval.

use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use async_trait::async_trait;
use axum::{extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, routing::{get, post}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
#[cfg(feature="burn-local")]
use sha2::{Digest, Sha256};
use crate::{api::{access_feed, authorize}, error::{Error, Result}, model::Item, AppState};

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/items/{id}/embed", post(embed_item))
        .route("/api/v1/feeds/{slug}/semantic", get(semantic_search))
}

struct RemoteProvider { client:reqwest::Client, url:String, token:Option<String> }
#[async_trait]
impl EmbeddingProvider for RemoteProvider {
    fn name(&self)->&'static str{"remote"}
    async fn embed(&self,text:&str)->Result<Vec<f32>>{
        let mut request=self.client.post(&self.url).json(&json!({"input":text}));
        if let Some(token)=&self.token{request=request.bearer_auth(token);}
        let response=request.send().await?.error_for_status()?;let value:Value=response.json().await?;
        value.get("embedding").or_else(||value.pointer("/data/0/embedding")).and_then(|v|serde_json::from_value(v.clone()).ok()).filter(|v:&Vec<f32>|!v.is_empty()).ok_or_else(||Error::Invalid("embedding service returned no vector".into()))
    }
}

#[cfg(feature="burn-local")]
struct BurnLocalProvider;
#[cfg(feature="burn-local")]
#[async_trait]
impl EmbeddingProvider for BurnLocalProvider {
    fn name(&self)->&'static str{"burn-local-v1"}
    async fn embed(&self,text:&str)->Result<Vec<f32>>{
        use burn::{backend::NdArray, tensor::Tensor};
        let mut values=vec![0.0f32;384];
        for token in text.split(|c:char|!c.is_alphanumeric()).filter(|v|!v.is_empty()){
            let digest=Sha256::digest(token.to_lowercase().as_bytes());let index=u16::from_le_bytes([digest[0],digest[1]])as usize%values.len();let sign=if digest[2]&1==0{1.0}else{-1.0};values[index]+=sign;
        }
        let norm=values.iter().map(|v|v*v).sum::<f32>().sqrt().max(f32::EPSILON);for v in &mut values{*v/=norm;}
        let device=Default::default();let tensor=Tensor::<NdArray<f32>,1>::from_floats(values.as_slice(),&device);
        tensor.into_data().to_vec::<f32>().map_err(|e|Error::Internal(e.to_string()))
    }
}

fn provider(state:&AppState)->Result<Arc<dyn EmbeddingProvider>>{
    match state.config.embedding_provider.as_str(){
        "remote"=>{let url=state.config.embedding_url.clone().ok_or_else(||Error::Config("SIDEFEED_EMBEDDING_URL is required for remote embeddings".into()))?;Ok(Arc::new(RemoteProvider{client:state.http.clone(),url,token:state.config.embedding_token.clone()}))}
        #[cfg(feature="burn-local")]
        "burn-local"=>Ok(Arc::new(BurnLocalProvider)),
        #[cfg(not(feature="burn-local"))]
        "burn-local"=>Err(Error::Config("rebuild with --features burn-local".into())),
        "disabled"=>Err(Error::Config("embedding provider is disabled".into())),
        other=>Err(Error::Config(format!("unknown embedding provider: {other}"))),
    }
}

async fn embed_item(State(state):State<AppState>,headers:HeaderMap,Path(id):Path<String>)->Result<(StatusCode,Json<Value>)>{
    authorize(&state,&headers)?;let item=state.store.item(&id).await?;let provider=provider(&state)?;let vector=provider.embed(&item_text(&item)).await?;state.store.put_embedding(&item.id,provider.name(),&vector).await?;Ok((StatusCode::CREATED,Json(json!({"item_id":item.id,"provider":provider.name(),"dimensions":vector.len()}))))
}

#[derive(Deserialize)] struct SemanticQuery {q:String,limit:Option<u32>}
async fn semantic_search(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(query):Query<SemanticQuery>)->Result<Json<Vec<Value>>>{
    access_feed(&state,&headers,&slug).await?;if query.q.trim().is_empty(){return Err(Error::Invalid("q is required".into()));}
    let provider=provider(&state)?;let needle=provider.embed(&query.q).await?;let mut merged:HashMap<String,(f32,Item)>=HashMap::new();
    for (id,vector) in state.store.embeddings(provider.name()).await?{if vector.len()!=needle.len(){continue;}let item=state.store.item(&id).await?;if state.store.item_in_feed(&slug,&item).await?{merged.insert(id,(cosine(&needle,&vector),item));}}
    if let Ok(lexical)=state.store.search(&slug,&query.q,100).await{for (rank,item) in lexical.into_iter().enumerate(){let boost=0.35/(rank as f32+1.0);merged.entry(item.id.clone()).and_modify(|v|v.0+=boost).or_insert((boost,item));}}
    let mut scored:Vec<_>=merged.into_values().collect();scored.sort_by(|a,b|b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));scored.truncate(query.limit.unwrap_or(20).clamp(1,100)as usize);
    Ok(Json(scored.into_iter().map(|(score,item)|json!({"score":score,"item":item})).collect()))
}

fn item_text(item:&Item)->String{format!("{}\n{}\n{}",item.title.as_deref().unwrap_or(""),item.summary.as_deref().unwrap_or(""),item.content.as_deref().unwrap_or(""))}
fn cosine(a:&[f32],b:&[f32])->f32{let dot=a.iter().zip(b).map(|(x,y)|x*y).sum::<f32>();let an=a.iter().map(|v|v*v).sum::<f32>().sqrt();let bn=b.iter().map(|v|v*v).sum::<f32>().sqrt();if an==0.0||bn==0.0{0.0}else{dot/(an*bn)}}

#[cfg(test)]
mod tests{use super::*;#[test]fn cosine_orders_vectors(){assert!(cosine(&[1.0,0.0],&[1.0,0.0])>cosine(&[1.0,0.0],&[0.0,1.0]));}}
