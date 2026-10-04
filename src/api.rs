use std::{cmp::Ordering, collections::{hash_map::Entry, HashMap}, convert::Infallible, time::Duration};
use async_stream::stream;
use axum::{body::{Body, Bytes}, extract::{Path, Query, State}, http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri}, response::{IntoResponse, Response, Sse, sse::Event}, routing::{get, post}, Json, Router};
use include_dir::{include_dir, Dir};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};
use crate::{error::{Error, Result}, ingest, model::{EnrichedItem, Feed, Item, ItemWithFeed, Page, Source}, AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/docs", get(api_docs)).route("/openapi.json", get(openapi))
        .route("/healthz", get(health)).route("/readyz", get(ready))
        .route("/api/v1/public/feeds", get(public_feeds))
        .route("/api/v1/tags", get(public_tags))
        .route("/api/v1/recent", get(recent))
        .route("/api/v1/updates", get(updates))
        .route("/api/v1/search", get(search_all))
        .route("/api/v1/bookmarks", get(list_bookmarks))
        .route("/api/v1/items/{id}/bookmark", post(add_bookmark).delete(remove_bookmark))
        .route("/api/v1/items/{id}/similar", get(similar))
        .route("/api/v1/sources", get(list_sources).post(create_source))
        .route("/api/v1/sources/{id}/poll", post(poll_source))
        .route("/api/v1/import/opml", post(import_opml))
        .route("/api/v1/feeds", get(list_feeds).post(create_feed))
        .route("/api/v1/feeds/{slug}/sources/{source_id}", post(attach_source))
        .route("/api/v1/feeds/{slug}/items", get(feed_items))
        .route("/api/v1/feeds/{slug}/search", get(search))
        .route("/api/v1/feeds/{slug}/stream", get(feed_stream))
        .route("/feeds/{file}", get(feed_output))
        .route("/feeds/{slug}/newsletter", get(newsletter))
        .route("/feeds/{slug}/thread.json", get(social_thread))
        .merge(crate::federation::router()).merge(crate::ai::router()).merge(crate::enrich::router())
        // Anything the API did not claim belongs to the reader, including its
        // client-side routes.
        .fallback(serve_ui)
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT,Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http()).with_state(state)
}

// The reader is built by Vite into `web/` and committed, so the binary carries
// the whole interface. `SIDEFEED_WEB_DIR` overrides any of it from disk, which
// is how the UI is edited on a running host without a rebuild.
static DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/web/dist");

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "woff2" => "font/woff2",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Disk first so edits land immediately, then the copy compiled into the binary.
async fn read_asset(state: &AppState, path: &str) -> Option<Vec<u8>> {
    if let Some(directory) = &state.config.web_dir {
        if let Ok(bytes) = tokio::fs::read(directory.join(path)).await { return Some(bytes); }
    }
    DIST.get_file(path).map(|file| file.contents().to_vec())
}

async fn asset_response(state: &AppState, path: &str) -> Option<Response> {
    let bytes = read_asset(state, path).await?;
    Some(with_bytes(bytes, content_type_for(path)))
}

/// Serves the reader: a real file when the path names one, otherwise the app
/// shell so client-side routes deep-link. API paths keep answering JSON.
async fn serve_ui(State(state): State<AppState>, method: Method, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") || path.starts_with("federation/") {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response();
    }
    if method == Method::GET || method == Method::HEAD {
        if !path.is_empty() {
            if let Some(response) = asset_response(&state, path).await { return response; }
        }
        if let Some(response) = asset_response(&state, "index.html").await { return response; }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

/// Small text documents that stay service-owned and overridable: the API
/// reference and its spec.
async fn text_asset(state: &AppState, name: &str, fallback: &'static str, content_type: &'static str) -> Response {
    if let Some(directory) = &state.config.web_dir {
        if let Ok(bytes) = tokio::fs::read(directory.join(name)).await { return with_bytes(bytes, content_type); }
    }
    with_type(fallback.to_owned(), content_type)
}

async fn api_docs(State(state): State<AppState>) -> Response { text_asset(&state, "docs.html", include_str!("web/docs.html"), "text/html; charset=utf-8").await }
async fn openapi(State(state): State<AppState>) -> Response { text_asset(&state, "openapi.json", include_str!("web/openapi.json"), "application/json; charset=utf-8").await }

/// Public feed index for the read-only reader interface. Management views keep
/// requiring the admin token; this projection exposes only feeds marked public.
#[derive(Serialize)] struct PublicFeed { slug:String, title:String, description:Option<String> }
async fn public_feeds(State(state):State<AppState>)->Result<Json<Vec<PublicFeed>>>{
    Ok(Json(state.store.feeds().await?.into_iter().filter(|feed|feed.public).map(|feed|PublicFeed{slug:feed.slug,title:feed.title,description:feed.description}).collect()))
}

#[derive(Deserialize)] struct RecentQuery {hours:Option<u32>,limit:Option<u32>,tag:Option<String>,matching:Option<String>,host:Option<String>}
#[derive(Serialize)] struct RecentItem {#[serde(flatten)] item:Item,feed:String,feed_title:String,tags:Vec<String>,ai_summary:Option<String>,score:f32}

/// Tag counts across every public feed in a window, for the tag browser when no
/// single feed is selected.
#[derive(Deserialize)] struct TagQuery {hours:Option<u32>,limit:Option<u32>}
async fn public_tags(State(state):State<AppState>,Query(q):Query<TagQuery>)->Result<Json<Value>>{
    let hours=q.hours.unwrap_or(24*7).clamp(1,24*90);
    let limit=q.limit.unwrap_or(40).clamp(1,200);
    let since=(Utc::now()-chrono::Duration::hours(hours as i64)).to_rfc3339();
    let tags=state.store.public_tags(&since,limit).await?;
    Ok(Json(json!(tags.into_iter().map(|(tag,count)|json!({"tag":tag,"count":count})).collect::<Vec<_>>())))
}

/// What happened lately across every public feed, ranked by recency and source
/// variety so one busy source cannot fill the list. Token-free, like the other
/// public reads.
async fn recent(State(state):State<AppState>,Query(q):Query<RecentQuery>)->Result<Json<Vec<RecentItem>>>{
    let hours=q.hours.unwrap_or(48).clamp(1,336);
    let limit=q.limit.unwrap_or(40).clamp(1,200);
    let (tags,all)=parse_tags(q.tag.as_deref(),q.matching.as_deref());
    let host=host_param(q.host.as_deref());
    let since=(Utc::now()-chrono::Duration::hours(hours as i64)).to_rfc3339();
    let rows=state.store.recent_items(&since,limit.saturating_mul(4).min(400),&tags,all,host.as_deref()).await?;
    let now=Utc::now();
    let mut per_source:HashMap<String,usize>=HashMap::new();
    let mut scored:Vec<(f32,ItemWithFeed)>=rows.into_iter().map(|row|{
        let age_hours=chrono::DateTime::parse_from_rfc3339(&row.item.published_at)
            .map(|stamp|(now-stamp.with_timezone(&Utc)).num_minutes() as f32/60.0)
            .unwrap_or(hours as f32);
        let recency=(1.0-(age_hours/hours as f32)).clamp(0.0,1.0);
        let count=per_source.entry(row.feed_slug.clone()).or_insert(0);
        let variety=1.0/(1.0+*count as f32);
        *count+=1;
        (recency*0.7+variety*0.3,row)
    }).collect();
    scored.sort_by(|left,right|right.0.partial_cmp(&left.0).unwrap_or(Ordering::Equal));
    let ids:Vec<String>=scored.iter().map(|(_,row)|row.item.id.clone()).collect();
    let atoms=crate::enrich::atoms_for(&state,&ids).await.unwrap_or_default();
    let mut seen:HashMap<String,usize>=HashMap::new();
    let mut items=Vec::new();
    for (score,row) in scored {
        let count=seen.entry(row.feed_slug.clone()).or_insert(0);
        if *count>=6{continue;}
        *count+=1;
        let derived=atoms.get(&row.item.id).cloned().unwrap_or_default();
        items.push(RecentItem{feed:row.feed_slug,feed_title:row.feed_title,item:row.item,tags:derived.tags,ai_summary:derived.summary,score});
        if items.len()>=limit as usize{break;}
    }
    Ok(Json(items))
}

#[derive(Deserialize)] struct UpdatesQuery {hours:Option<u32>}

/// Per-category rollup of what moved recently, for the updates page. Token-free
/// because it only ever describes public feeds.
async fn updates(State(state):State<AppState>,Query(q):Query<UpdatesQuery>)->Result<Json<crate::enrich::Updates>>{
    Ok(Json(crate::enrich::updates(&state,q.hours.unwrap_or(48).clamp(1,168)).await?))
}

#[derive(Deserialize)] struct SearchAllQuery {q:String,hours:Option<u32>,limit:Option<u32>,feed:Option<String>,tag:Option<String>,matching:Option<String>,order:Option<String>,host:Option<String>}
#[derive(Serialize)] struct FeedHit {#[serde(flatten)] item:Item,feed:String,feed_title:String,tags:Vec<String>,ai_summary:Option<String>}

/// User text becomes an FTS5 expression: each word is quoted and prefix-matched,
/// so punctuation and operators in a search box cannot turn into syntax errors.
fn fts_query(raw:&str)->String{
    raw.split_whitespace()
        .map(|term|term.chars().filter(|character|character.is_alphanumeric()||*character=='-'||*character=='_').collect::<String>())
        .filter(|term|!term.is_empty())
        .map(|term|format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Search across every public feed, or one feed when `feed` is given. Without
/// `hours` the window is everything, so a back-catalogue stays findable.
async fn search_all(State(state):State<AppState>,Query(q):Query<SearchAllQuery>)->Result<Json<Vec<FeedHit>>>{
    let query=fts_query(&q.q);
    if query.is_empty(){return Err(Error::Invalid("q is required".into()));}
    let limit=q.limit.unwrap_or(40).clamp(1,200);
    let (tags,all)=parse_tags(q.tag.as_deref(),q.matching.as_deref());
    let oldest=q.order.as_deref()==Some("oldest");
    let since=match q.hours{
        Some(hours)=>(Utc::now()-chrono::Duration::hours(hours.clamp(1,24*365*20) as i64)).to_rfc3339(),
        None=>"1970-01-01T00:00:00Z".to_string(),
    };
    let feed=q.feed.as_deref().map(str::trim).filter(|value|!value.is_empty());
    let host=host_param(q.host.as_deref());
    let rows=state.store.search_public(crate::store::SearchOptions{
        query:&query,since:&since,limit,feed,host:host.as_deref(),tags:&tags,all_tags:all,oldest_first:oldest,
    }).await?;
    let ids:Vec<String>=rows.iter().map(|row|row.item.id.clone()).collect();
    let atoms=crate::enrich::atoms_for(&state,&ids).await.unwrap_or_default();
    Ok(Json(rows.into_iter().map(|row|{
        let derived=atoms.get(&row.item.id).cloned().unwrap_or_default();
        FeedHit{feed:row.feed_slug,feed_title:row.feed_title,item:row.item,tags:derived.tags,ai_summary:derived.summary}
    }).collect()))
}

#[derive(Serialize)] struct SavedHit { #[serde(flatten)] item: Item, feed:String, feed_title:String, saved_at:String, tags:Vec<String>, ai_summary:Option<String> }
#[derive(Serialize)] struct SimilarHit { #[serde(flatten)] item: Item, feed:String, feed_title:String, tags:Vec<String>, ai_summary:Option<String>, score:f32 }

/// The reader's saved list. Token-free, like the rest of the public read path.
async fn list_bookmarks(State(state):State<AppState>)->Result<Json<Vec<SavedHit>>>{
    let rows=state.store.saved_items(500).await?;
    let ids:Vec<String>=rows.iter().map(|row|row.item.id.clone()).collect();
    let atoms=crate::enrich::atoms_for(&state,&ids).await.unwrap_or_default();
    Ok(Json(rows.into_iter().map(|row|{let derived=atoms.get(&row.item.id).cloned().unwrap_or_default();SavedHit{item:row.item,feed:row.feed_slug,feed_title:row.feed_title,saved_at:row.saved_at,tags:derived.tags,ai_summary:derived.summary}}).collect()))
}

/// How many items one node will save. Bounds a token-free write route.
const BOOKMARK_LIMIT: i64 = 2000;
async fn add_bookmark(State(state):State<AppState>,Path(id):Path<String>)->Result<StatusCode>{state.store.bookmark(&id,BOOKMARK_LIMIT).await?;Ok(StatusCode::NO_CONTENT)}
async fn remove_bookmark(State(state):State<AppState>,Path(id):Path<String>)->Result<StatusCode>{state.store.unbookmark(&id).await?;Ok(StatusCode::NO_CONTENT)}

#[derive(Deserialize)] struct SimilarQuery {limit:Option<u32>}

fn host_of(url:Option<&str>)->Option<String>{ url::Url::parse(url?).ok()?.host_str().map(str::to_string) }

/// Related items for one article, blending four signals: the same source, the
/// same host, shared derived tags, and shared title keywords. Each signal adds
/// to one score per candidate, so an item that matches several ways ranks
/// first. Token-free, because it only ever describes public items.
async fn similar(State(state):State<AppState>,Path(id):Path<String>,Query(q):Query<SimilarQuery>)->Result<Json<Vec<SimilarHit>>>{
    let item=state.store.item(&id).await?;
    let limit=q.limit.unwrap_or(8).clamp(1,50);
    let pool=limit.saturating_mul(8).min(200);
    let model=crate::enrich::enricher(&state).map(|enricher|enricher.name().to_string()).ok();
    let tags=match &model{Some(name)=>state.store.atoms(&id,name).await.map(|atoms|atoms.tags).unwrap_or_default(),None=>Vec::new()};
    let mut scored:HashMap<String,(f32,ItemWithFeed)>=HashMap::new();
    if let Some(name)=&model{
        for (overlap,row) in state.store.similar_by_tags(&id,name,&tags,pool).await?{add_candidate(&mut scored,row,overlap as f32*2.0);}
    }
    if let Some(source_id)=item.source_id.as_deref(){
        for row in state.store.similar_by_source(source_id,&id,pool).await?{add_candidate(&mut scored,row,3.0);}
    }
    if let Some(host)=host_of(item.url.as_deref()){
        for row in state.store.similar_by_host(&host,&id,pool).await?{add_candidate(&mut scored,row,2.0);}
    }
    let terms=crate::enrich::keywords(item.title.as_deref().unwrap_or(""));
    if !terms.is_empty(){
        let query=terms.iter().map(|term|format!("\"{term}\"")).collect::<Vec<_>>().join(" OR ");
        for row in state.store.similar_by_terms(&query,&id,pool).await?{add_candidate(&mut scored,row,1.0);}
    }
    let mut ranked:Vec<(f32,ItemWithFeed)>=scored.into_values().collect();
    ranked.sort_by(|left,right|right.0.partial_cmp(&left.0).unwrap_or(Ordering::Equal).then_with(||right.1.item.published_at.cmp(&left.1.item.published_at)));
    ranked.truncate(limit as usize);
    let ids:Vec<String>=ranked.iter().map(|(_,row)|row.item.id.clone()).collect();
    let atoms=crate::enrich::atoms_for(&state,&ids).await.unwrap_or_default();
    Ok(Json(ranked.into_iter().map(|(score,row)|{let derived=atoms.get(&row.item.id).cloned().unwrap_or_default();SimilarHit{item:row.item,feed:row.feed_slug,feed_title:row.feed_title,tags:derived.tags,ai_summary:derived.summary,score}}).collect()))
}

/// Adds one signal's weight to a candidate, or inserts it the first time the
/// item appears.
fn add_candidate(scored:&mut HashMap<String,(f32,ItemWithFeed)>,row:ItemWithFeed,weight:f32){
    match scored.entry(row.item.id.clone()){
        Entry::Occupied(mut found)=>{found.get_mut().0+=weight;}
        Entry::Vacant(slot)=>{slot.insert((weight,row));}
    }
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

#[derive(Deserialize)] struct PageQuery {limit:Option<u32>,cursor:Option<String>,tag:Option<String>,matching:Option<String>,host:Option<String>}

/// Accepts either a bare host or a whole URL, because the UI offers "more from
/// this site" from an article link.
fn host_param(value: Option<&str>) -> Option<String> {
    let raw = value?.trim().to_lowercase();
    if raw.is_empty() { return None; }
    let host = raw.split("//").last().unwrap_or(&raw).split('/').next().unwrap_or(&raw);
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() { None } else { Some(host.to_string()) }
}

/// Tags arrive comma separated, matching what the reader sends after the user
/// clicks a few. `matching=all` switches from any-of to every-of.
fn parse_tags(tag: Option<&str>, matching: Option<&str>) -> (Vec<String>, bool) {
    let tags = tag.unwrap_or_default().split(',')
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .take(12)
        .collect();
    (tags, matching == Some("all"))
}

async fn feed_items(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<PageQuery>)->Result<Json<Page<EnrichedItem>>>{
    access_feed(&state,&headers,&slug).await?;let limit=q.limit.unwrap_or(50).clamp(1,200);
    let (tags,all)=parse_tags(q.tag.as_deref(),q.matching.as_deref());
    let host=host_param(q.host.as_deref());
    let items=state.store.feed_items_tagged(&slug,limit,q.cursor.as_deref(),&tags,all,host.as_deref()).await?;
    let next_cursor=if items.len()==limit as usize{items.last().map(|i|i.published_at.clone())}else{None};
    // One query for the whole page's derived artifacts, then one small cache fill.
    let ids:Vec<String>=items.iter().map(|item|item.id.clone()).collect();
    let atoms=crate::enrich::atoms_for(&state,&ids).await.unwrap_or_default();
    let page=items.into_iter().map(|item|{let found=atoms.get(&item.id).cloned().unwrap_or_default();EnrichedItem::new(item,found)}).collect();
    Ok(Json(Page{items:page,next_cursor}))
}
#[derive(Deserialize)] struct SearchQuery {q:String,limit:Option<u32>}
async fn search(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>,Query(q):Query<SearchQuery>)->Result<Json<Vec<Item>>>{access_feed(&state,&headers,&slug).await?;if q.q.trim().is_empty(){return Err(Error::Invalid("q is required".into()));}Ok(Json(state.store.search(&slug,&q.q,q.limit.unwrap_or(30).clamp(1,100)).await?))}

async fn feed_stream(State(state):State<AppState>,headers:HeaderMap,Path(slug):Path<String>)->Result<Sse<impl futures_util::Stream<Item=std::result::Result<Event,Infallible>>>>{
    access_feed(&state,&headers,&slug).await?;let mut receiver=state.events.subscribe();let store=state.store.clone();
    let events=stream!{loop{match receiver.recv().await{Ok(item)=>if store.item_in_feed(&slug,&item).await.unwrap_or(false){yield Ok(Event::default().event("item").json_data(&item).unwrap_or_else(|_|Event::default().event("error")));},Err(tokio::sync::broadcast::error::RecvError::Lagged(n))=>yield Ok(Event::default().event("lagged").data(n.to_string())),Err(_)=>break}}};
    Ok(Sse::new(events).keep_alive(axum::response::sse::KeepAlive::default()))
}

// axum 0.8 rejects a suffix after a path parameter within one segment, so the
// public /feeds/{slug}.rss and /feeds/{slug}.json URLs are captured by a single
// route and dispatched on the extension of the requested file here.
async fn feed_output(State(state):State<AppState>,headers:HeaderMap,Path(file):Path<String>)->Result<Response>{
    let (slug,extension)=file.rsplit_once('.').ok_or(Error::NotFound)?;
    let slug=slug.to_owned();
    match extension{
        "rss"=>rss_feed(State(state),headers,Path(slug)).await,
        "json"=>Ok(json_feed(State(state),headers,Path(slug)).await?.into_response()),
        _=>Err(Error::NotFound),
    }
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

pub(crate) async fn access_feed(state:&AppState,headers:&HeaderMap,slug:&str)->Result<Feed>{let feed=state.store.feed(slug).await?;if !feed.public{authorize(state,headers)?;}Ok(feed)}
pub(crate) fn authorize(state:&AppState,headers:&HeaderMap)->Result<()>{let Some(expected)=&state.config.admin_token else{return Ok(())};let supplied=headers.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));if supplied==Some(expected.as_str()){Ok(())}else{Err(Error::Unauthorized)}}
fn validate_slug(slug:&str)->Result<()>{if slug.is_empty()||slug.len()>64||!slug.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-'){Err(Error::Invalid("slug must contain lowercase letters, digits, or hyphens".into()))}else{Ok(())}}
fn esc(value:&str)->String{value.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\'',"&#39;")}
fn with_type(body:String,content_type:&'static str)->Response{with_bytes(body.into_bytes(),content_type)}
fn with_bytes(body:Vec<u8>,content_type:&'static str)->Response{let mut response=Body::from(body).into_response();response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static(content_type));response}
