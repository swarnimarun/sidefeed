use std::{io::Cursor, net::IpAddr};
use chrono::Utc;
use feed_rs::model::Entry;
use futures_util::StreamExt;
use quick_xml::{events::Event, Reader};
use reqwest::{header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, LOCATION}, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use url::Url;
use uuid::Uuid;
use crate::{error::{Error, Result}, model::{NewItem, Source}, AppState};

pub async fn poll_loop(state: AppState) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        ticker.tick().await;
        if let Err(error) = poll_due(&state).await { tracing::error!(%error, "poll cycle failed"); }
    }
}

pub async fn poll_due(state: &AppState) -> Result<()> {
    crate::federation::sync_due(state).await;
    for source in state.store.due_sources(32).await? {
        if let Err(error) = poll_source(state, &source).await {
            tracing::warn!(source_id=%source.id, %error, "source poll failed");
            let next = (Utc::now() + chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15))).to_rfc3339();
            state.store.update_source_fetch(&source.id, None, None, None, Some(&error.to_string()), &next).await?;
        }
    }
    if state.config.retention_days > 0 {
        let before=(Utc::now()-chrono::Duration::days(state.config.retention_days as i64)).to_rfc3339();
        state.store.prune_items(&before).await?;
    }
    Ok(())
}

pub async fn poll_source(state: &AppState, source: &Source) -> Result<usize> {
    let owner = Uuid::new_v4().to_string();
    let resource = format!("source:{}", source.id);
    if !state.store.acquire_lease(&resource, &owner, state.config.fetch_timeout.as_secs() as i64 + 30).await? { return Ok(0); }
    let result = poll_source_inner(state, source).await;
    state.store.release_lease(&resource, &owner).await?;
    result
}

async fn poll_source_inner(state: &AppState, source: &Source) -> Result<usize> {
    // ---- lane-ingest: kind dispatch (Tasks 3-4) ----
    // ActivityPub sources poll outboxes through `ap`; webhook sources are
    // push-only (ingress stores their items), so a poll is a no-op rather
    // than a fetch error. `raw-json` dispatch lands in Task 4.
    if source.kind == "activitypub" { return crate::ap::poll_actor(state, source).await; }
    if source.kind == "webhook" { return Ok(0); }
    let mut url = Url::parse(&source.url).map_err(|e| Error::Invalid(format!("invalid source URL: {e}")))?;
    let mut response = None;
    for hop in 0..=5 {
        validate_public_url(&url).await?;
        let mut request = state.http.get(url.clone()).header("Accept", "application/atom+xml, application/rss+xml, application/feed+json, application/activity+json, application/json;q=0.8, */*;q=0.1");
        if hop == 0 {
            if let Some(etag) = &source.etag { request = request.header(IF_NONE_MATCH, etag); }
            if let Some(value) = &source.last_modified { request = request.header(IF_MODIFIED_SINCE, value); }
        }
        let current = request.send().await?;
        // 304 sits inside the 3xx range. A conditional GET answering "unchanged"
        // is not a redirect and carries no Location header to follow.
        if current.status() == StatusCode::NOT_MODIFIED { response = Some(current); break; }
        if current.status().is_redirection() {
            let location = current.headers().get(LOCATION).and_then(|v| v.to_str().ok()).ok_or_else(|| Error::Invalid("redirect missing Location".into()))?;
            url = url.join(location).map_err(|e| Error::Invalid(format!("invalid redirect: {e}")))?;
            continue;
        }
        response = Some(current); break;
    }
    let response = response.ok_or_else(|| Error::Invalid("too many redirects".into()))?;
    let next = (Utc::now() + chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15))).to_rfc3339();
    if response.status() == StatusCode::NOT_MODIFIED {
        state.store.update_source_fetch(&source.id, None, None, None, None, &next).await?;
        return Ok(0);
    }
    if !response.status().is_success() { return Err(Error::Invalid(format!("origin returned {}", response.status()))); }
    if response.content_length().is_some_and(|n| n > state.config.max_response_bytes as u64) { return Err(Error::Invalid("source response is too large".into())); }
    let etag = header(&response, ETAG);
    let modified = header(&response, LAST_MODIFIED);
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > state.config.max_response_bytes { return Err(Error::Invalid("source response is too large".into())); }
        bytes.extend_from_slice(&chunk);
    }
    let (title, items) = parse_document(&bytes, &content_type, &url)?;
    let mut count = 0;
    for candidate in items {
        let item = state.store.upsert_item(Some(&source.id), &candidate).await?;
        let _ = state.events.send(item);
        count += 1;
    }
    state.store.update_source_fetch(&source.id, title.as_deref(), etag.as_deref(), modified.as_deref(), None, &next).await?;
    Ok(count)
}

fn header(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

pub fn parse_document(bytes: &[u8], content_type: &str, base: &Url) -> Result<(Option<String>, Vec<NewItem>)> {
    let trimmed = bytes.iter().copied().find(|byte| !byte.is_ascii_whitespace());
    if content_type.contains("json") || trimmed == Some(b'{') || trimmed == Some(b'[') { return parse_json(bytes, base); }
    let feed = feed_rs::parser::parse(Cursor::new(bytes)).map_err(|e| Error::Invalid(format!("unsupported feed: {e}")))?;
    let title = feed.title.map(|v| v.content);
    // Feeds do ship relative links, and a relative link is useless once it has
    // left the page it was written for: resolve every item against the feed URL.
    let items = feed.entries.iter().map(|entry| {
        let mut item = normalize_entry(entry);
        item.url = item.url.map(|href| resolve_url(base, &href));
        item
    }).collect();
    Ok((title, items))
}

fn resolve_url(base: &Url, href: &str) -> String {
    base.join(href).map(|url| url.to_string()).unwrap_or_else(|_| href.to_string())
}

fn normalize_entry(entry: &Entry) -> NewItem {
    let (published_at, date_source) = dated(entry.published.map(|v| v.to_rfc3339()), entry.updated.map(|v| v.to_rfc3339()));
    let body = entry.content.as_ref().and_then(|v| v.body.clone());
    let summary = entry.summary.as_ref().map(|v| v.content.clone());
    NewItem {
        external_id: if entry.id.is_empty() { entry.links.first().map(|l| l.href.clone()).unwrap_or_else(|| Uuid::new_v4().to_string()) } else { entry.id.clone() },
        url: prefer_article(entry.links.first().map(|v| v.href.clone()), body.as_deref().or(summary.as_deref())),
        title: entry.title.as_ref().map(|v| v.content.clone()),
        summary,
        content: body,
        author: entry.authors.first().map(|v| v.name.clone()),
        published_at, date_source,
        tags: entry.categories.iter().map(|v| v.term.clone()).collect(), raw: None, visibility: "public".into(),
    }
}

/// A feed's own date and where it came from. Prefer the posted date, fall back
/// to last-modified, and only then to the fetch time — recorded as `fetched` so
/// the reader can label it instead of showing it as the post date.
pub(crate) fn dated(published: Option<String>, updated: Option<String>) -> (String, String) {
    if let Some(value) = published { return (value, "published".into()); }
    if let Some(value) = updated { return (value, "updated".into()); }
    (Utc::now().to_rfc3339(), "fetched".into())
}

// ---------------------------------------------------------------- link cleanup
//
// Aggregators post a story and its discussion as two links. Reddit puts the
// comments page in the item link and the article in the body as a "[link]"
// anchor, so without this the reader offers the thread as "the original". The
// body keeps the thread link, which is what the reader shows as "comments".

fn is_discussion(url: &str) -> bool {
    let lowered = url.to_lowercase();
    (lowered.contains("reddit.com") && lowered.contains("/comments/"))
        || lowered.contains("news.ycombinator.com/item")
        || (lowered.contains("lobste.rs") && lowered.contains("/s/"))
        || lowered.contains("tildes.net/~")
}

/// `out.reddit.com/t3_x?url=<target>` wrappers hide where a link really goes.
fn unwrap_redirect(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else { return url.to_string() };
    if parsed.host_str() != Some("out.reddit.com") { return url.to_string(); }
    parsed.query_pairs().find(|(key, _)| key == "url").map(|(_, value)| value.into_owned()).unwrap_or_else(|| url.to_string())
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    for needle in [format!("{name}=\""), format!("{name}='"), format!("{}=\"", name.to_uppercase())] {
        if let Some(start) = tag.find(&needle) {
            let rest = &tag[start + needle.len()..];
            let quote = if needle.ends_with('\'') { '\'' } else { '"' };
            if let Some(end) = rest.find(quote) { return Some(rest[..end].to_string()); }
        }
    }
    None
}

/// The first http(s) link in a body that is not itself a discussion page.
fn first_article_link(html: &str) -> Option<String> {
    let mut rest = html;
    while let Some(position) = rest.find("<a ") {
        rest = &rest[position + 3..];
        let end = rest.find('>')?;
        if let Some(href) = attribute(&rest[..end], "href") {
            let resolved = unwrap_redirect(&href.replace("&amp;", "&"));
            if resolved.starts_with("http") && !is_discussion(&resolved) { return Some(resolved); }
        }
        rest = &rest[end..];
    }
    None
}

fn prefer_article(url: Option<String>, body: Option<&str>) -> Option<String> {
    let url = url?;
    let resolved = unwrap_redirect(&url);
    if !is_discussion(&resolved) { return Some(resolved); }
    body.and_then(first_article_link).or(Some(resolved))
}

#[derive(Deserialize)]
struct JsonFeed { title: Option<String>, #[serde(default)] items: Vec<JsonFeedItem> }
#[derive(Deserialize)]
struct JsonFeedItem {
    id: String, url: Option<String>, title: Option<String>, summary: Option<String>, content_html: Option<String>, content_text: Option<String>,
    date_published: Option<String>, date_modified: Option<String>, #[serde(default)] tags: Vec<String>, #[serde(default)] authors: Vec<JsonAuthor>, author: Option<JsonAuthor>,
}
#[derive(Deserialize)] struct JsonAuthor { name: Option<String> }

fn parse_json(bytes: &[u8], base: &Url) -> Result<(Option<String>, Vec<NewItem>)> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| Error::Invalid(format!("invalid JSON feed: {e}")))?;
    if value.get("version").and_then(Value::as_str).is_some_and(|v| v.contains("jsonfeed.org")) {
        let feed: JsonFeed = serde_json::from_value(value).map_err(|e| Error::Invalid(e.to_string()))?;
        let items = feed.items.into_iter().map(|i| {
            let (published_at, date_source) = dated(i.date_published, i.date_modified);
            NewItem {
                external_id: i.id, url: i.url, title: i.title, summary: i.summary, content: i.content_html.or(i.content_text),
                author: i.authors.first().or(i.author.as_ref()).and_then(|a| a.name.clone()),
                published_at, date_source, tags: i.tags,
                raw: None, visibility: "public".into(),
            }
        }).collect();
        return Ok((feed.title, items));
    }
    parse_activitypub(value, base)
}

fn parse_activitypub(value: Value, base: &Url) -> Result<(Option<String>, Vec<NewItem>)> {
    let title = value.get("name").and_then(Value::as_str).map(str::to_owned);
    let list = value.get("orderedItems").or_else(|| value.get("items")).and_then(Value::as_array)
        .cloned().unwrap_or_else(|| vec![value.clone()]);
    let mut items = Vec::new();
    for activity in list {
        // The object mapping lives in `ap` so the generic ingest path and the
        // outbox poller can never drift; this keeps the outer list handling.
        let object = activity.get("object").filter(|v| v.is_object()).unwrap_or(&activity);
        if let Some(item) = crate::ap::normalize_object(object, &activity, base) { items.push(item); }
    }
    Ok((title, items))
}

pub(crate) fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str().map(str::to_owned).or_else(|| v.get("href").and_then(Value::as_str).map(str::to_owned)))
}

pub fn parse_opml(bytes: &[u8]) -> Result<Vec<(String, Option<String>)>> {
    let mut reader = Reader::from_reader(bytes);
    let mut result = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(tag)) | Ok(Event::Start(tag)) if tag.name().as_ref() == b"outline" => {
                let mut url = None; let mut title = None;
                for attr in tag.attributes().flatten() {
                    let value = attr.decode_and_unescape_value(reader.decoder()).map_err(|e| Error::Invalid(e.to_string()))?.into_owned();
                    match attr.key.as_ref() { b"xmlUrl" => url = Some(value), b"title" | b"text" if title.is_none() => title = Some(value), _ => {} }
                }
                if let Some(url) = url { result.push((url, title)); }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Invalid(format!("invalid OPML: {e}"))),
            _ => {}
        }
    }
    Ok(result)
}

pub async fn validate_public_url(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") { return Err(Error::Invalid("only http(s) sources are allowed".into())); }
    let host = url.host_str().ok_or_else(|| Error::Invalid("URL has no host".into()))?;
    let port = url.port_or_known_default().ok_or_else(|| Error::Invalid("URL has no port".into()))?;
    let addresses: Vec<_> = tokio::net::lookup_host((host, port)).await.map_err(|e| Error::Invalid(format!("cannot resolve source host: {e}")))?.collect();
    if addresses.is_empty() || addresses.iter().any(|a| blocked_ip(a.ip())) { return Err(Error::Invalid("private or non-routable source address rejected".into())); }
    Ok(())
}

fn blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local() || v.is_broadcast() || v.is_unspecified() || v.is_multicast() || v.octets()[0] == 0,
        IpAddr::V6(v) => v.is_loopback() || v.is_unspecified() || v.is_multicast() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reddit_items_point_at_the_article_not_the_thread() {
        let thread = "https://www.reddit.com/r/rust/comments/abc/title/";
        let body = "<table><tr><td><a href=\"https://out.reddit.com/t3_abc?url=https%3A%2F%2Fexample.com%2Fpost&amp;token=x\">[link]</a></td>\
                    <td><a href=\"https://www.reddit.com/r/rust/comments/abc/title/\">[comments]</a></td></tr></table>";
        assert_eq!(prefer_article(Some(thread.into()), Some(body)).as_deref(), Some("https://example.com/post"));
    }

    #[test]
    fn a_thread_without_an_article_keeps_its_own_link() {
        let thread = "https://news.ycombinator.com/item?id=42";
        assert_eq!(prefer_article(Some(thread.into()), Some("<a href=\"https://news.ycombinator.com/item?id=42\">Comments</a>")).as_deref(), Some(thread));
    }

    #[test]
    fn ordinary_links_and_redirects_are_left_alone() {
        assert_eq!(prefer_article(Some("https://example.com/post".into()), None).as_deref(), Some("https://example.com/post"));
        assert_eq!(unwrap_redirect("https://example.com/post"), "https://example.com/post");
    }

    #[test]
    fn relative_item_links_resolve_against_the_feed_url() {
        let base = Url::parse("https://blog.example.com/feed.xml").unwrap();
        assert_eq!(resolve_url(&base, "/blog/pixel-art"), "https://blog.example.com/blog/pixel-art");
        assert_eq!(resolve_url(&base, "posts/one"), "https://blog.example.com/posts/one");
        assert_eq!(resolve_url(&base, "https://other.example.com/x"), "https://other.example.com/x");
    }
    #[test]
    fn parses_json_feed() {
        let base = Url::parse("https://example.com/feed").unwrap();
        let (_, items) = parse_document(br#"{"version":"https://jsonfeed.org/version/1.1","title":"x","items":[{"id":"1","content_text":"hello"}]}"#, "application/feed+json", &base).unwrap();
        assert_eq!(items[0].external_id, "1");
    }
    #[test]
    fn parses_opml_urls() {
        let feeds = parse_opml(br#"<opml><body><outline text="Example" xmlUrl="https://example.com/rss"/></body></opml>"#).unwrap();
        assert_eq!(feeds[0].0, "https://example.com/rss");
    }
    #[test]
    fn blocks_private_addresses() { assert!(blocked_ip("127.0.0.1".parse().unwrap())); assert!(!blocked_ip("1.1.1.1".parse().unwrap())); }
}
