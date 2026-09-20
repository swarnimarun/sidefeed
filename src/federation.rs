//! Pull-based cache exchange between explicitly trusted Sidefeed nodes.

use axum::{extract::{OriginalUri, Path, Query, State}, http::{HeaderMap, Method, StatusCode}, routing::{get, post}, Json, Router};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use crate::{api::authorize, error::{Error, Result}, ingest::validate_public_url, model::{Item, NewItem, Peer}, AppState};

type HmacSha256 = Hmac<Sha256>;
const PEER_TIME: &str = "x-sidefeed-timestamp";
const PEER_SIGNATURE: &str = "x-sidefeed-signature";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/peers", get(list_peers).post(create_peer))
        .route("/api/v1/peers/{id}/sync", post(sync_peer))
        .route("/federation/v1/items", get(export_items))
}

#[derive(Deserialize)] struct PeerInput { base_url:String, shared_secret:String }
async fn create_peer(State(state):State<AppState>,headers:HeaderMap,Json(input):Json<PeerInput>)->Result<(StatusCode,Json<Peer>)>{
    authorize(&state,&headers)?;if input.shared_secret.len()<32{return Err(Error::Invalid("peer secret must be at least 32 characters".into()));}
    let url=url::Url::parse(&input.base_url).map_err(|e|Error::Invalid(e.to_string()))?;validate_public_url(&url).await?;
    Ok((StatusCode::CREATED,Json(state.store.create_peer(url.as_str(),&input.shared_secret).await?)))
}
async fn list_peers(State(state):State<AppState>,headers:HeaderMap)->Result<Json<Vec<Peer>>>{authorize(&state,&headers)?;Ok(Json(state.store.peers().await?))}

#[derive(Deserialize)] struct ExportQuery { since:Option<String>, limit:Option<u32> }
async fn export_items(State(state):State<AppState>,headers:HeaderMap,method:Method,OriginalUri(uri):OriginalUri,Query(query):Query<ExportQuery>)->Result<Json<Vec<Item>>>{
    verify_request(&state,&headers,&method,uri.path_and_query().map(|v|v.as_str()).unwrap_or(uri.path())).await?;
    let since=query.since.as_deref().unwrap_or("1970-01-01T00:00:00Z");
    DateTime::parse_from_rfc3339(since).map_err(|_|Error::Invalid("since must be RFC3339".into()))?;
    Ok(Json(state.store.public_items_since(since,query.limit.unwrap_or(100).clamp(1,state.config.peer_max_items)).await?))
}

async fn sync_peer(State(state):State<AppState>,headers:HeaderMap,Path(id):Path<String>)->Result<Json<serde_json::Value>>{
    authorize(&state,&headers)?;let peer=state.store.peer(&id).await?;let merged=sync_one(&state,&peer).await?;Ok(Json(serde_json::json!({"merged":merged})))
}

pub async fn sync_due(state:&AppState){
    let cutoff=Utc::now()-chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15));
    match state.store.peers().await{Ok(peers)=>for peer in peers{let due=peer.last_sync_at.as_deref().and_then(|v|DateTime::parse_from_rfc3339(v).ok()).map_or(true,|last|last.with_timezone(&Utc)<cutoff);if due{if let Err(error)=sync_one(state,&peer).await{tracing::warn!(peer_id=%peer.id,%error,"peer sync failed");}}},Err(error)=>tracing::warn!(%error,"could not list peers")}
}

async fn sync_one(state:&AppState,peer:&Peer)->Result<usize>{
    let since=peer.last_sync_at.clone().unwrap_or_else(||"1970-01-01T00:00:00Z".into());
    let mut url=url::Url::parse(&format!("{}/federation/v1/items",peer.base_url)).map_err(|e|Error::Invalid(e.to_string()))?;
    url.query_pairs_mut().append_pair("since",&since).append_pair("limit",&state.config.peer_max_items.to_string());
    validate_public_url(&url).await?;
    let path=url[url::Position::BeforePath..].to_owned();let timestamp=Utc::now().timestamp().to_string();let signature=sign(&peer.shared_secret,&timestamp,"GET",&path)?;
    let response=state.http.get(url).header(PEER_TIME,&timestamp).header(PEER_SIGNATURE,signature).send().await?;
    if !response.status().is_success(){return Err(Error::Invalid(format!("peer returned {}",response.status())));}
    let mut bytes=Vec::new();let mut body=response.bytes_stream();
    while let Some(chunk)=body.next().await{let chunk=chunk?;if bytes.len()+chunk.len()>state.config.max_response_bytes{return Err(Error::Invalid("peer response is too large".into()));}bytes.extend_from_slice(&chunk);}
    let items:Vec<Item>=serde_json::from_slice(&bytes).map_err(|e|Error::Invalid(format!("invalid peer response: {e}")))?;
    let mut merged=0;
    for item in items {let candidate=NewItem{external_id:item.external_id,url:item.url,title:item.title,summary:item.summary,content:item.content,author:item.author,published_at:item.published_at,tags:serde_json::from_str(&item.tags_json).unwrap_or_default(),raw:item.raw_json.and_then(|v|serde_json::from_str(&v).ok()),visibility:item.visibility};let stored=state.store.upsert_item(None,&candidate).await?;let _=state.events.send(stored);merged+=1;}
    state.store.touch_peer(&peer.id).await?;Ok(merged)
}

async fn verify_request(state:&AppState,headers:&HeaderMap,method:&Method,path:&str)->Result<()> {
    let timestamp=value(headers,PEER_TIME)?;let signature=value(headers,PEER_SIGNATURE)?;
    let sent: i64=timestamp.parse().map_err(|_|Error::Unauthorized)?;if (Utc::now().timestamp()-sent).abs()>300{return Err(Error::Unauthorized);}
    let expected=hex::decode(signature).map_err(|_|Error::Unauthorized)?;let message=format!("{timestamp}\n{}\n{path}",method.as_str());
    for peer in state.store.peers().await? {let mut mac=HmacSha256::new_from_slice(peer.shared_secret.as_bytes()).map_err(|_|Error::Unauthorized)?;mac.update(message.as_bytes());if mac.verify_slice(&expected).is_ok(){return Ok(());}}
    Err(Error::Unauthorized)
}
fn value<'a>(headers:&'a HeaderMap,name:&str)->Result<&'a str>{headers.get(name).and_then(|v|v.to_str().ok()).ok_or(Error::Unauthorized)}
fn sign(secret:&str,timestamp:&str,method:&str,path:&str)->Result<String>{let mut mac=HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_|Error::Internal("invalid HMAC key".into()))?;mac.update(format!("{timestamp}\n{method}\n{path}").as_bytes());Ok(hex::encode(mac.finalize().into_bytes()))}

#[cfg(test)]
mod tests {use super::*;#[test]fn signatures_are_stable(){assert_eq!(sign(&"x".repeat(32),"1","GET","/items").unwrap(),sign(&"x".repeat(32),"1","GET","/items").unwrap());}}
