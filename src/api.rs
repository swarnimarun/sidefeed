use std::{convert::Infallible, time::Duration};
use async_stream::stream;
use axum::{body::Bytes, extract::{Path, Query, State}, http::{header, HeaderMap, HeaderValue, StatusCode}, response::{IntoResponse, Response, Sse, sse::Event}, routing::{get, post}, Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};
use crate::{error::{Error, Result}, ingest, model::{Feed, Item, Page, Source}, AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health)).route("/readyz", get(ready))
        .route("/api/v1/sources", get(list_sources).post(create_source))
        .route("/api/v1/sources/{id}/poll", post(poll_source))
        .route("/api/v1/import/opml", post(import_opml))
        .route("/api/v1/feeds", get(list_feeds).post(create_feed))
        .route("/api/v1/feeds/{slug}/sources/{source_id}", post(attach_source))
        .route("/api/v1/feeds/{slug}/items", get(feed_items))
        .route("/api/v1/feeds/{slug}/search", get(search))
        .route("/api/v1/feeds/{slug}/stream", get(feed_stream))
        .route("/feeds/{slug}.rss", get(rss_feed))
        .route("/feeds/{slug}.json", get(json_feed))
        .route("/feeds/{slug}/newsletter", get(newsletter))
        .route("/feeds/{slug}/thread.json", get(social_thread))
        .merge(crate::federation::router()).merge(crate::ai::router())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http()).with_state(state)
}

async fn health() -> Json<Value> { Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION")})) }
async fn ready(State(state): State<AppState>) -> Result<Json<Value>> { state.store.ping().await?; Ok(Json(json!({"status":"ready"}))) }

#[derive(Deserialize)] struct SourceInput { url: String, #[serde(default="auto_kind")] kind: String, title: Option<String> }
fn auto_kind() -> String { "auto".into() }
async fn create_source(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<SourceInput>) -> Result<(StatusCode,Json<Source>)> {
    authorize(&state,&headers)?; let url=url::Url::parse(&input.url).map_err(|e|Error::Invalid(e.to_string()))?; ingest::validate_public_url(&url).await?;
    Ok((StatusCode::CREATED,Json(state.store.create_source(url.as_str(),&input.kind,input.title.as_deref()).await?)))
}
async fn list_sources(State(state):State<AppState>,headers:HeaderMap)->Result<Json<Vec<Source>>>{authorize(&state,&headers)?;Ok(Json(state.store.sources().await?))}
async fn poll_source(State(state):State<AppState>,headers:HeaderMap,Path(id):Path<String>)->Result<Json<Value>>{authorize(&state,&headers)?;let source=state.store.source(&id).await?;let imported=ingest::poll_source(&state,&source).await?;Ok(Json(json!({"imported":imported})))}
async fn import_opml(State(state):State<AppState>,headers:HeaderMap,body:Bytes)->Result<(StatusCode,Json<Value>)>{
    authorize(&state,&headers)?;let feeds=ingest::parse_opml(&body)?;let mut created=Vec::new();let mut skipped=0;
    for (url,title) in feeds {let parsed=url::Url::parse(&url).map_err(|e|Error::Invalid(format!("invalid OPML URL: {e}")))?;ingest::validate_public_url(&parsed).await?;match state.store.create_source(parsed.as_str(),"auto",title.as_deref()).await{Ok(s)=>created.push(s),Err(Error::Conflict(_))=>skipped+=1,Err(e)=>return Err(e)}}
    Ok((StatusCode::CREATED,Json(json!({"created":created,"skipped":skipped}))))
}

#[derive(Deserialize)] struct FeedInput { slug:String,title:String,description:Option<String>,include_terms:Option<String>,exclude_terms:Option<String>,#[serde(default)]public:bool }
async fn create_feed(State(state):State<AppState>,headers:HeaderMap,Json(input):Json<FeedInput>)->Result<(StatusCode,Json<Feed>)>{authorize(&state,&headers)?;validate_slug(&input.slug)?;let feed=state.store.create_feed(&input.slug,&input.title,input.description.as_deref(),input.include_terms.as_deref(),input.exclude_terms.as_deref(),input.public).await?;Ok((StatusCode::CREATED,Json(feed)))}
async fn list_feeds(State(state):State<AppState>,headers:HeaderMap)->Result<Json<Vec<Feed>>>{authorize(&state,&headers)?;Ok(Json(state.store.feeds().await?))}
async fn attach_source(State(state):State<AppState>,headers:HeaderMap,Path((slug,source_id)):Path<(String,String)>)->Result<StatusCode>{authorize(&state,&headers)?;state.store.attach_source(&slug,&source_id).await?;Ok(StatusCode::NO_CONTENT)}

#[derive(Deserialize)] struct PageQuery {limit:Option<u32>,cursor:Option<String>}
async fn feed_items(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<PageQuery>)->Result<Json<Page<Item>>>{access_feed(&state,&headers,&slug).await?;let limit=q.limit.unwrap_or(50).clamp(1,200);let items=state.store.feed_items(&slug,limit,q.cursor.as_deref()).await?;let next_cursor=if items.len()==limit as usize{items.last().map(|i|i.published_at.clone())}else{None};Ok(Json(Page{items,next_cursor}))}
#[derive(Deserialize)] struct SearchQuery {q:String,limit:Option<u32>}
async fn search(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<SearchQuery>)->Result<Json<Vec<Item>>>{access_feed(&state,&headers,&slug).await?;if q.q.trim().is_empty(){return Err(Error::Invalid("q is required".into()));}Ok(Json(state.store.search(&slug,&q.q,q.limit.unwrap_or(30).clamp(1,100)).await?))}

async fn feed_stream(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>)->Result<Sse<impl futures_util::Stream<Item=std::result::Result<Event,Infallible>>>>{
    access_feed(&state,&headers,&slug).await?;let mut receiver=state.events.subscribe();let store=state.store.clone();
    let events=stream!{loop{match receiver.recv().await{Ok(item)=>if store.item_in_feed(&slug,&item).await.unwrap_or(false){yield Ok(Event::default().event("item").json_data(&item).unwrap_or_else(|_|Event::default().event("error")));},Err(tokio::sync::broadcast::error::RecvError::Lagged(n))=>yield Ok(Event::default().event("lagged").data(n.to_string())),Err(_)=>break}}};
    Ok(Sse::new(events).keep_alive(axum::response::sse::KeepAlive::default()))
}

async fn rss_feed(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>)->Result<Response>{
    let feed=access_feed(&state,&headers,&slug).await?;let items=state.store.feed_items(&slug,100,None).await?;let mut xml=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel><title>{}</title><link>{}/feeds/{}.rss</link><description>{}</description>",esc(&feed.title),esc(&state.config.public_url),esc(&slug),esc(feed.description.as_deref().unwrap_or("")));
    for i in items{xml.push_str(&format!("<item><guid isPermaLink=\"false\">{}</guid><title>{}</title><link>{}</link><description>{}</description><pubDate>{}</pubDate></item>",esc(&i.external_id),esc(i.title.as_deref().unwrap_or("")),esc(i.url.as_deref().unwrap_or("")),esc(i.summary.as_deref().or(i.content.as_deref()).unwrap_or("")),esc(&i.published_at)));}xml.push_str("</channel></rss>");Ok(with_type(xml,"application/rss+xml; charset=utf-8"))
}
async fn json_feed(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>)->Result<Json<Value>>{
    let feed=access_feed(&state,&headers,&slug).await?;let items=state.store.feed_items(&slug,100,None).await?;Ok(Json(json!({"version":"https://jsonfeed.org/version/1.1","title":feed.title,"home_page_url":state.config.public_url,"feed_url":format!("{}/feeds/{}.json",state.config.public_url,slug),"items":items.into_iter().map(|i|json!({"id":i.external_id,"url":i.url,"title":i.title,"summary":i.summary,"content_html":i.content,"date_published":i.published_at,"authors":i.author.map(|name|vec![json!({"name":name})]).unwrap_or_default()})).collect::<Vec<_>>() })))
}
#[derive(Deserialize)] struct OutputQuery {limit:Option<u32>,#[serde(default)]format:Option<String>}
async fn newsletter(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<OutputQuery>)->Result<Response>{
    let feed=access_feed(&state,&headers,&slug).await?;let items=state.store.feed_items(&slug,q.limit.unwrap_or(20).clamp(1,100),None).await?;
    if q.format.as_deref()==Some("text"){let mut out=format!("{}\n{}\n\n",feed.title,feed.description.unwrap_or_default());for i in items{out.push_str(&format!("- {}\n  {}\n  {}\n\n",i.title.unwrap_or_else(||"Untitled".into()),i.summary.unwrap_or_default(),i.url.unwrap_or_default()));}return Ok(with_type(out,"text/plain; charset=utf-8"));}
    let mut html=format!("<!doctype html><html><body><main><h1>{}</h1><p>{}</p>",esc(&feed.title),esc(feed.description.as_deref().unwrap_or("")));for i in items{html.push_str(&format!("<article><h2><a href=\"{}\">{}</a></h2><p>{}</p></article>",esc(i.url.as_deref().unwrap_or("#")),esc(i.title.as_deref().unwrap_or("Untitled")),esc(i.summary.as_deref().or(i.content.as_deref()).unwrap_or(""))));}html.push_str("</main></body></html>");Ok(with_type(html,"text/html; charset=utf-8"))
}
async fn social_thread(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<OutputQuery>)->Result<Json<Value>>{
    access_feed(&state,&headers,&slug).await?;let items=state.store.feed_items(&slug,q.limit.unwrap_or(10).clamp(1,25),None).await?;Ok(Json(json!({"generated_at":Utc::now().to_rfc3339(),"posts":items.into_iter().map(|i|{let title=i.title.unwrap_or_else(||"New item".into());let url=i.url.unwrap_or_default();let mut text=format!("{}\n{}",title,url);if text.chars().count()>280{text=text.chars().take(279).collect();text.push('…');}json!({"text":text,"item_id":i.id})}).collect::<Vec<_>>() })))
}

async fn access_feed(state:&AppState,headers:&HeaderMap,slug:&str)->Result<Feed>{let feed=state.store.feed(slug).await?;if !feed.public{authorize(state,headers)?;}Ok(feed)}
pub(crate) fn authorize(state:&AppState,headers:&HeaderMap)->Result<()>{let Some(expected)=&state.config.admin_token else{return Ok(())};let supplied=headers.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));if supplied==Some(expected.as_str()){Ok(())}else{Err(Error::Unauthorized)}}
fn validate_slug(slug:&str)->Result<()>{if slug.is_empty()||slug.len()>64||!slug.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-'){Err(Error::Invalid("slug must contain lowercase letters, digits, or hyphens".into()))}else{Ok(())}}
fn esc(value:&str)->String{value.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\'',"&#39;")}
fn with_type(body:String,content_type:&'static str)->Response{let mut response=body.into_response();response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static(content_type));response}
