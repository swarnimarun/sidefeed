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
    // ActivityPub sources poll outboxes through `ap`; `raw-json` sources go
    // through the generic REST poller in `channels`. Webhook sources are
    // push-only (ingress stores their items), so a poll is a no-op rather
    // than a fetch error.
    if source.kind == "activitypub" { return crate::ap::poll_actor(state, source).await; }
    if source.kind == "raw-json" {
        let stored = state.store.source_config(&source.id).await?.unwrap_or(crate::model::SourceConfig {
            source_id: source.id.clone(), kind: "raw-json".into(), config_json: "{}".into(),
        });
        let config: crate::channels::RawJsonConfig = serde_json::from_str(&stored.config_json)
            .map_err(|e| Error::Invalid(format!("invalid raw-json config: {e}")))?;
        return crate::channels::poll_raw_json(state, source, &config).await;
    }
    if source.kind == "webhook" { return Ok(0); }
    let url = Url::parse(&source.url).map_err(|e| Error::Invalid(format!("invalid source URL: {e}")))?;
    let mut fetched = fetch_validated(state, url, source.etag.as_deref(), source.last_modified.as_deref()).await?;
    let next = (Utc::now() + chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15))).to_rfc3339();
    if fetched.not_modified {
        state.store.update_source_fetch(&source.id, None, None, None, None, &next).await?;
        return Ok(0);
    }
    // A source URL that returns a web page is probably the site homepage, not
    // the feed: follow one `<link rel="alternate">` discovery hop (Task 4).
    if is_html(&fetched.content_type, &fetched.bytes) {
        let text = String::from_utf8_lossy(&fetched.bytes);
        if let Some(discovered) = discover_feed_url(&text, &fetched.url) {
            let target = Url::parse(&discovered).map_err(|e| Error::Invalid(format!("invalid discovered feed URL: {e}")))?;
            fetched = fetch_validated(state, target, None, None).await?;
            if fetched.not_modified {
                state.store.update_source_fetch(&source.id, None, None, None, None, &next).await?;
                return Ok(0);
            }
        }
    }
    let (title, mut items) = parse_document(&fetched.bytes, &fetched.content_type, &fetched.url)?;
    // JSON Feed archives page through `next_url`: follow up to two further
    // pages (three total), each re-checked and re-capped like the first.
    let mut paging = next_url_of(&fetched.bytes);
    for _ in 0..2 {
        let Some(page_url) = paging.take() else { break; };
        let target = Url::parse(&page_url).map_err(|e| Error::Invalid(format!("invalid next_url: {e}")))?;
        let page = fetch_validated(state, target, None, None).await?;
        if page.not_modified { break; }
        let (_, mut more) = parse_document(&page.bytes, &page.content_type, &page.url)?;
        paging = next_url_of(&page.bytes);
        items.append(&mut more);
    }
    let mut count = 0;
    for candidate in items {
        let item = state.store.upsert_item(Some(&source.id), &candidate).await?;
        let _ = state.events.send(item);
        count += 1;
    }
    state.store.update_source_fetch(&source.id, title.as_deref(), fetched.etag.as_deref(), fetched.modified.as_deref(), None, &next).await?;
    Ok(count)
}

/// One SSRF-checked, size-bounded GET with redirect following. Conditional
/// validators apply to the first hop only; every hop (including the first)
/// is checked against the private-network denylist before it is sent.
/// `Ok` with `not_modified` is a 304 answer to the validators.
pub(crate) struct Fetched {
    pub url: Url,
    pub not_modified: bool,
    pub etag: Option<String>,
    pub modified: Option<String>,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub(crate) async fn fetch_validated(state: &AppState, mut url: Url, etag: Option<&str>, modified: Option<&str>) -> Result<Fetched> {
    let mut response = None;
    for hop in 0..=5 {
        validate_public_url(&url).await?;
        let mut request = state.http.get(url.clone()).header("Accept", "application/atom+xml, application/rss+xml, application/feed+json, application/activity+json, application/json;q=0.8, */*;q=0.1");
        if hop == 0 {
            if let Some(value) = etag { request = request.header(IF_NONE_MATCH, value); }
            if let Some(value) = modified { request = request.header(IF_MODIFIED_SINCE, value); }
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
    if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(Fetched { url, not_modified: true, etag: None, modified: None, content_type: String::new(), bytes: Vec::new() });
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
    Ok(Fetched { url, not_modified: false, etag, modified, content_type, bytes })
}

fn is_html(content_type: &str, bytes: &[u8]) -> bool {
    if content_type.contains("html") { return true; }
    if content_type.contains("json") || content_type.contains("xml") { return false; }
    matches!(bytes.iter().copied().find(|byte| !byte.is_ascii_whitespace()), Some(b'<'))
}

/// The `next_url` of a JSON Feed document, if it carries one.
fn next_url_of(bytes: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(bytes).ok()?.get("next_url").and_then(Value::as_str).map(str::to_owned)
}

/// Feed URL discovery (Task 4): the `href` of a `<link rel="alternate">`
/// tag whose type is a known feed type, resolved against the page URL.
/// Scans the raw markup so malformed pages still resolve; only the head's
/// alternates qualify, never stylesheets or icons.
pub fn discover_feed_url(html: &str, base: &Url) -> Option<String> {
    const TYPES: [&str; 3] = ["application/feed+json", "application/atom+xml", "application/rss+xml"];
    let lower = html.to_lowercase();
    let mut rest = 0;
    while let Some(start) = lower[rest..].find("<link") {
        let start = rest + start;
        // `<link` must end at a separator so `<linkfoo` never matches.
        if !lower[start + 5..].starts_with([' ', '\t', '\n', '\r', '/', '>']) {
            rest = start + 5;
            continue;
        }
        let end = lower[start..].find('>').map(|i| start + i)?;
        let tag = &html[start..end.min(html.len())];
        let rel = attribute(tag, "rel").unwrap_or_default().to_lowercase();
        let kind = attribute(tag, "type").unwrap_or_default().to_lowercase();
        if rel.split_whitespace().any(|token| token == "alternate") && TYPES.contains(&kind.as_str()) {
            if let Some(href) = attribute(tag, "href") {
                if let Ok(resolved) = base.join(&href) {
                    return Some(resolved.to_string());
                }
            }
        }
        rest = end + 1;
    }
    None
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
    // Real-world JSON Feeds omit `id` on briefs; fall back to the URL and
    // only then to a generated id so a sloppy entry still ingests stably.
    #[serde(default)] id: Option<String>, url: Option<String>, title: Option<String>, summary: Option<String>, content_html: Option<String>, content_text: Option<String>,
    date_published: Option<String>, date_modified: Option<String>, #[serde(default)] tags: Vec<String>, #[serde(default)] authors: Vec<JsonAuthor>, author: Option<JsonAuthor>,
}
#[derive(Deserialize)] struct JsonAuthor { name: Option<String> }

fn parse_json(bytes: &[u8], base: &Url) -> Result<(Option<String>, Vec<NewItem>)> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| Error::Invalid(format!("invalid JSON feed: {e}")))?;
    if value.get("version").and_then(Value::as_str).is_some_and(|v| v.contains("jsonfeed.org")) {
        let feed: JsonFeed = serde_json::from_value(value).map_err(|e| Error::Invalid(e.to_string()))?;
        let items = feed.items.into_iter().map(|i| {
            let (published_at, date_source) = dated(i.date_published, i.date_modified);
            let external_id = i.id.or_else(|| i.url.clone()).unwrap_or_else(|| Uuid::new_v4().to_string());
            NewItem {
                external_id, url: i.url, title: i.title, summary: i.summary, content: i.content_html.or(i.content_text),
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

/// At most this many outlines are accepted from one OPML document; above it
/// the import is rejected instead of creating an unbounded set of sources.
const MAX_OPML_SOURCES: usize = 500;

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
                if result.len() > MAX_OPML_SOURCES {
                    return Err(Error::Invalid("OPML item limit is 500".into()));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Invalid(format!("invalid OPML: {e}"))),
            _ => {}
        }
    }
    if result.len() > MAX_OPML_SOURCES {
        return Err(Error::Invalid("OPML item limit is 500".into()));
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
    fn json_feed_entries_without_ids_fall_back_to_their_url() {
        let base = Url::parse("https://example.com/feed").unwrap();
        let (_, items) = parse_document(br#"{"version":"https://jsonfeed.org/version/1.1","items":[{"url":"https://example.com/p/1","content_text":"hi"}]}"#, "application/feed+json", &base).unwrap();
        assert_eq!(items[0].external_id, "https://example.com/p/1");
    }
    #[test]
    fn discovery_finds_feed_alternates_and_ignores_the_rest() {
        let base = Url::parse("https://example.com/blog/").unwrap();
        let html = r#"<html><head>
            <link rel="stylesheet" href="/app.css">
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
            <link rel="alternate" type="application/feed+json" href="https://cdn.example/feed.json">
        </head></html>"#;
        // The first alternate wins; relative hrefs resolve against the page.
        assert_eq!(discover_feed_url(html, &base).as_deref(), Some("https://example.com/feed.xml"));
        let json_only = r#"<head><link rel='alternate' type='application/feed+json' href='/f.json'></head>"#;
        assert_eq!(discover_feed_url(json_only, &base).as_deref(), Some("https://example.com/f.json"));
        let none = r#"<head><link rel="alternate" type="text/html" href="/other"></head>"#;
        assert_eq!(discover_feed_url(none, &base), None);
        assert_eq!(discover_feed_url("<p>no links here</p>", &base), None);
    }
    #[test]
    fn parses_opml_urls() {
        let feeds = parse_opml(br#"<opml><body><outline text="Example" xmlUrl="https://example.com/rss"/></body></opml>"#).unwrap();
        assert_eq!(feeds[0].0, "https://example.com/rss");
    }
    #[test]
    fn opml_rejects_more_than_five_hundred_outlines() {
        let mut doc = String::from("<opml><body>");
        for i in 0..501 {
            doc.push_str(&format!(r#"<outline text="f{i}" xmlUrl="https://example.com/{i}.xml"/>"#));
        }
        doc.push_str("</body></opml>");
        assert!(parse_opml(doc.as_bytes()).is_err());
    }
    #[test]
    fn blocks_private_addresses() { assert!(blocked_ip("127.0.0.1".parse().unwrap())); assert!(!blocked_ip("1.1.1.1".parse().unwrap())); }
}
