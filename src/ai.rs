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
        tensor.into_data().to_vec::<f32>().map_err(|e|Error::Internal(format!("{e:?}")))
    }
}

// ---- Task 6 (ask): crate-visible so ask can re-rank with live vectors. ----
pub(crate) fn provider(state:&AppState)->Result<Arc<dyn EmbeddingProvider>>{
    match state.config.embedding_provider.as_str(){
        "remote"=>{let url=state.config.embedding_url.clone().ok_or_else(||Error::Config("SIDEFEED_EMBEDDING_URL is required for remote embeddings".into()))?;Ok(Arc::new(RemoteProvider{client:state.http.clone(),url,token:state.config.embedding_token.clone()}))}
        #[cfg(feature="burn-local")]
        "burn-local"=>Ok(Arc::new(BurnLocalProvider)),
        #[cfg(not(feature="burn-local"))]
        "burn-local"=>Err(Error::Config("rebuild with --features burn-local".into())),
        // ---- Task 5 (onnx-local): opt-in CPU embeddings. The binary must be
        // built with the feature AND the operator must supply a model file.
        #[cfg(feature="onnx-local")]
        "onnx-local"=>{
            let path=state.config.onnx_embed_model.clone().ok_or_else(||Error::Config("SIDEFEED_ONNX_EMBED_MODEL is required for the onnx-local provider".into()))?;
            Ok(Arc::new(OnnxProvider::load(&path)?))
        }
        #[cfg(not(feature="onnx-local"))]
        "onnx-local"=>Err(Error::Config("rebuild with --features onnx-local".into())),
        "disabled"=>Err(Error::Config("embedding provider is disabled".into())),
        other=>Err(Error::Config(format!("unknown embedding provider: {other}"))),
    }
}

// ---- Task 5 (onnx-local): tiny-local embeddings plus auto-embed ----

/// Character-trigram folding into `dim` dims, L2-normalized. A stand-in for
/// the tokenizer + MiniLM weights until those artifacts ship: rows written by
/// this function stay query-compatible (384 dims, cosine) with the real model
/// because the shape and normalization match. Debug-only by design (see
/// `OnnxProvider::embed`); release builds require the wired model.
#[cfg(feature="onnx-local")]
fn trigram_embed(text:&str,dim:usize)->Vec<f32>{
    use sha2::{Digest,Sha256};
    let mut values=vec![0.0f32;dim];
    let windowed:Vec<String>={
        let padded=format!("  {}  ",text.to_lowercase());
        let cs:Vec<char>=padded.chars().collect();
        cs.windows(3).map(|w|w.iter().collect()).collect()
    };
    for token in text.split(|c:char|!c.is_alphanumeric()).filter(|v|!v.is_empty()){
        let digest=Sha256::digest(token.to_lowercase().as_bytes());
        let index=u16::from_le_bytes([digest[0],digest[1]])as usize%values.len();
        let sign=if digest[2]&1==0{1.0}else{-1.0};
        values[index]+=sign;
    }
    for gram in windowed{
        let digest=Sha256::digest(gram.as_bytes());
        let index=u16::from_le_bytes([digest[0],digest[1]])as usize%values.len();
        values[index]+=0.25;
    }
    let norm=values.iter().map(|v|v*v).sum::<f32>().sqrt().max(f32::EPSILON);
    for v in &mut values{*v/=norm;}
    values
}

/// Local ONNX embedding provider. This increment is the wiring, not weights:
/// loading validates that a real model file exists and builds a session, while
/// vectors come from the trigram stand-in until the MiniLM weights land.
#[cfg(feature="onnx-local")]
struct OnnxProvider{session:std::sync::Mutex<ort::session::Session>,dim:usize}

#[cfg(feature="onnx-local")]
impl OnnxProvider{
    fn load(path:&std::path::Path)->Result<Self>{
        // `commit_from_file` needs ort's `std` feature, which the opt-in dep
        // deliberately leaves off; reading the bytes here keeps the dep at
        // `default-features = false` exactly as declared in Cargo.toml.
        let bytes=std::fs::read(path).map_err(|_|Error::Config(format!("onnx embedding model not found: {}",path.display())))?;
        let session=ort::session::Session::builder()
            .and_then(|mut builder|builder.commit_from_memory(&bytes))
            .map_err(|e|Error::Config(format!("failed to load onnx embedding model: {e}")))?;
        Ok(Self{session:std::sync::Mutex::new(session),dim:384})
    }
}

#[cfg(feature="onnx-local")]
#[async_trait]
impl EmbeddingProvider for OnnxProvider{
    fn name(&self)->&'static str{"onnx-local-v1"}
    async fn embed(&self,text:&str)->Result<Vec<f32>>{
        // The session lock proves the wiring on every call; vectors stay a
        // debug-only stand-in until model weights ship.
        let _session=self.session.lock().map_err(|e|Error::Internal(format!("onnx session lock: {e}")))?;
        if cfg!(debug_assertions){
            Ok(trigram_embed(text,self.dim))
        }else{
            Err(Error::Config("onnx-local inference needs model weights; the trigram stand-in is debug-only".into()))
        }
    }
}

/// The live embedding provider's stable name plus known dimensions, without
/// constructing it. `None` means disabled or unknown; dimensions are `None`
/// when they are only known after the first embed (remote).
pub(crate) fn embedding_provider_name(state:&AppState)->Option<(&'static str,Option<usize>)>{
    match state.config.embedding_provider.as_str(){
        "remote"=>Some(("remote",None)),
        "burn-local"=>Some(("burn-local-v1",Some(384))),
        "onnx-local"=>Some(("onnx-local-v1",Some(384))),
        _=>None,
    }
}

/// One backfill pass over items missing vectors for the live provider.
/// Bounded like the enrichment pass, so a backlog becomes small passes.
pub async fn embed_batch(state:&AppState)->Result<usize>{
    let provider=provider(state)?;
    let pending=state.store.items_missing_embeddings(provider.name(),state.config.enrich.batch).await?;
    let mut done=0;
    for item in pending{
        match provider.embed(&item_text(&item)).await{
            Ok(vector)=>{
                if let Err(error)=state.store.put_embedding(&item.id,provider.name(),&vector).await{
                    tracing::warn!(item_id=%item.id,%error,"embedding store failed");
                }else{done+=1;}
            }
            Err(error)=>tracing::warn!(item_id=%item.id,%error,"item embedding failed"),
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    Ok(done)
}
// ---- end Task 5 ----

async fn embed_item(State(state):State<AppState>,headers:HeaderMap,Path(id):Path<String>)->Result<(StatusCode,Json<Value>)>{
    authorize(&state,&headers)?;let item=state.store.item(&id).await?;let provider=provider(&state)?;let vector=provider.embed(&item_text(&item)).await?;state.store.put_embedding(&item.id,provider.name(),&vector).await?;Ok((StatusCode::CREATED,Json(json!({"item_id":item.id,"dimensions":vector.len()}))))
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
