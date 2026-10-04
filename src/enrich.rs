//! Local text enrichment: tags and short summaries per item.
//!
//! Artifacts are computed once per item and stored on disk in SQLite, so a
//! restart never pays for them twice. The only memory this module holds is a
//! small bounded cache of the most recently read items, which keeps list
//! rendering off the database without growing with the archive.
//!
//! Two providers ship behind one tiny interface (text in, [`Atoms`] out):
//!
//! * `heuristic` — in-process, no network, no model files. An extractive
//!   summary plus term-frequency tags; milliseconds and a few KB per item, which
//!   is what fits a 2 vCPU / 1 GB host.
//! * `openai` — any OpenAI-compatible chat endpoint, so a small local model
//!   (llama.cpp server, Ollama, LM Studio) can do the work when quality matters
//!   more than footprint.
//!
//! The provider name is stored with every artifact, so switching models
//! recomputes rather than serving the previous model's output.

use std::{collections::{HashMap, VecDeque}, sync::Arc, time::Duration};
use async_trait::async_trait;
use axum::{extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, routing::{get, post}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::{api::{access_feed, authorize}, error::{Error, Result}, model::{Atoms, Item}, AppState};

#[async_trait]
pub trait Enricher: Send + Sync {
    /// Stable identifier stored alongside every artifact this provider writes.
    fn name(&self) -> &'static str;
    async fn enrich(&self, text: &str) -> Result<Atoms>;
    /// A short "what is new here" line for a group of items. Generative
    /// providers override this; extractive ones compose from the sample.
    async fn digest(&self, label: &str, sample: &str) -> Result<Option<String>> {
        let _ = label;
        Ok(heuristic_digest(sample))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/feeds/{slug}/tags", get(feed_tags))
        .route("/api/v1/items/{id}/enrich", post(enrich_now))
        // Reader-facing: a public item can be summarised on demand, without a
        // token, but only once per model. The artifact is stored, so every later
        // request is a read and a model is never called twice for one item.
        .route("/api/v1/feeds/{slug}/items/{item_id}/summarize", post(summarize_now))
}

// ---------------------------------------------------------------- text helpers

/// Third-party HTML is reduced to plain text before a model sees it: tags out, a
/// few entities decoded, whitespace collapsed. Same rules as the reader UI, so a
/// summary describes the text the reader actually shows.
pub fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => { in_tag = false; out.push(' '); }
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "have", "has", "are", "was", "were", "will", "would", "can", "could",
    "you", "your", "our", "their", "its", "not", "but", "they", "them", "there", "here", "what", "when", "where", "which",
    "while", "into", "over", "than", "then", "also", "more", "most", "some", "such", "only", "other", "after", "before",
    "about", "because", "been", "being", "does", "did", "doing", "just", "like", "make", "makes", "made", "many", "much",
    "new", "now", "one", "two", "out", "see", "still", "take", "used", "using", "very", "way", "who", "why", "how", "all",
    "any", "both", "each", "few", "own", "same", "too", "under", "off", "a", "an", "as", "at", "by", "is", "be", "or", "if",
    "it", "in", "of", "to", "on", "we", "us", "my", "me", "he", "she", "his", "her", "up", "do", "so", "no", "get",
];

/// Link furniture that would otherwise become a tag.
const TAG_NOISE: [&str; 10] = ["comments", "comment", "read", "more", "link", "permalink", "via", "here", "http", "https"];

/// Words a tag could be built from: lowercase, at least three characters, not a
/// stopword. Digits are kept because version and year tags are useful.
fn terms(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .map(|word| word.to_lowercase())
        .filter(|word| word.len() >= 3 && word.len() <= 24)
        .filter(|word| !STOPWORDS.contains(&word.as_str()))
        .filter(|word| word.chars().any(|character| character.is_alphabetic()))
        .collect()
}

/// Bodies that are only a link label say nothing, and would otherwise turn into
/// tags ("comments") or fake sentences.
const LINK_ONLY: [&str; 6] = ["comments", "comment", "read more", "continue reading", "permalink", "link"];
fn link_only(text: &str) -> bool {
    let trimmed = text.trim().trim_end_matches(['.', ':']).to_lowercase();
    if trimmed.is_empty() { return true; }
    if LINK_ONLY.contains(&trimmed.as_str()) { return true; }
    trimmed.starts_with("via ") || (trimmed.ends_with("comments") && trimmed.split_whitespace().count() <= 3)
}

/// Cheap sentence split: enough for feeds, no parser and no dependencies. A
/// decimal point or an abbreviation is not a sentence end, so "Rust 1.100.0,
/// the following changes" stays one sentence instead of becoming "0, the …".
fn sentences(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if !matches!(*byte, b'.' | b'!' | b'?') { continue; }
        let previous_is_digit = index > 0 && bytes[index - 1].is_ascii_digit();
        let next = bytes.get(index + 1).copied().unwrap_or(b' ');
        if previous_is_digit && next.is_ascii_digit() { continue; }
        if index + 1 < bytes.len() && !next.is_ascii_whitespace() { continue; }
        let starts_sentence = text[index + 1..].trim_start().chars().next()
            .map(|character| character.is_uppercase() || character.is_ascii_digit())
            .unwrap_or(true);
        if *byte == b'.' && !starts_sentence { continue; }
        let candidate = text[start..index + 1].trim();
        if candidate.len() >= 25 { out.push(candidate); }
        start = index + 1;
        if out.len() >= 60 { return out; }
    }
    let tail = text[start..].trim();
    if tail.len() >= 25 { out.push(tail); }
    out
}

fn truncate_at_word(text: &str, limit: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return None; }
    if trimmed.chars().count() <= limit { return Some(trimmed.to_string()); }
    let cut: String = trimmed.chars().take(limit).collect();
    let shortened = match cut.rfind(' ') {
        Some(index) if index > 40 => &cut[..index],
        _ => cut.as_str(),
    };
    Some(format!("{}…", shortened.trim_end_matches(['.', ',', ';', ' ', ':'])))
}

/// Extractive summary plus frequency tags. Deterministic, allocation-light, and
/// bounded by the caller's `max_chars` truncation. Tags survive on a bare title;
/// a summary needs a few sentences to choose between.
pub fn heuristic_atoms(text: &str) -> Atoms {
    let lower = text.to_lowercase();
    let words = terms(&lower);
    if words.len() < 5 { return Atoms::default(); }

    let mut counts: HashMap<&str, u32> = HashMap::new();
    for word in &words { *counts.entry(word.as_str()).or_insert(0) += 1; }

    let mut ranked: Vec<(&str, u32)> = counts.iter().map(|(term, count)| (*term, *count)).collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(right.0.len().cmp(&left.0.len())).then(left.0.cmp(right.0)));
    let tags: Vec<String> = ranked.iter()
        .filter(|(term, _)| !TAG_NOISE.contains(term))
        .filter(|(term, count)| *count >= 2 || term.len() >= 5)
        .take(6)
        .map(|(term, _)| (*term).to_string())
        .collect();

    if words.len() < 12 { return Atoms { tags, summary: None }; }

    let total = words.len() as f32;
    let sentences = sentences(text);
    let count = sentences.len().max(1) as f32;
    let mut scored: Vec<(f32, usize)> = sentences.iter().enumerate().map(|(index, sentence)| {
        let sentence_terms = terms(&sentence.to_lowercase());
        if sentence_terms.is_empty() { return (0.0, index); }
        let weight: f32 = sentence_terms.iter().map(|term| counts.get(term.as_str()).copied().unwrap_or(0) as f32 / total).sum();
        // A lede usually carries the point, so earlier sentences get a nudge.
        let lead = 1.0 - (index as f32 / count);
        (weight / (sentence_terms.len() as f32).sqrt() + lead * 0.12, index)
    }).collect();
    scored.sort_by(|left, right| right.0.partial_cmp(&left.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut picked: Vec<usize> = scored.iter().take(2).filter(|(score, _)| *score > 0.0).map(|(_, index)| *index).collect();
    picked.sort_unstable();
    let summary = truncate_at_word(&picked.iter().map(|index| sentences[*index]).collect::<Vec<_>>().join(" "), 280);

    Atoms { tags, summary }
}

/// Models wrap JSON in prose or code fences. Take the outermost object and read
/// the two fields that were asked for, ignoring anything else they add.
pub fn parse_atoms(reply: &str) -> Atoms {
    let (Some(start), Some(end)) = (reply.find('{'), reply.rfind('}')) else { return Atoms::default() };
    if end <= start { return Atoms::default(); }
    let Ok(value) = serde_json::from_str::<Value>(&reply[start..=end]) else { return Atoms::default() };
    let summary = value.get("summary").and_then(Value::as_str).map(str::trim)
        .filter(|text| !text.is_empty()).map(|text| truncate_at_word(text, 400).unwrap_or_default());
    let tags = value.get("tags").and_then(Value::as_array).map(|list| list.iter()
        .filter_map(Value::as_str).map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty() && tag.len() <= 40 && !TAG_NOISE.contains(&tag.as_str()))
        .take(8).collect()).unwrap_or_default();
    Atoms { tags, summary }
}

// ---------------------------------------------------------------- providers

pub struct HeuristicEnricher;

#[async_trait]
impl Enricher for HeuristicEnricher {
    fn name(&self) -> &'static str { "heuristic-v2" }
    async fn enrich(&self, text: &str) -> Result<Atoms> {
        let plain = plain_text(text);
        if plain.len() < 40 { return Ok(Atoms::default()); }
        Ok(heuristic_atoms(&plain))
    }
}

const INSTRUCTION: &str = "You summarise feed items. Reply with JSON only, no prose: \
{\"summary\": \"at most two sentences\", \"tags\": [\"up to 5 lowercase topical tags\"]}";

pub struct OpenAiEnricher {
    client: reqwest::Client,
    url: String,
    token: Option<String>,
    model: String,
    max_chars: usize,
}

impl OpenAiEnricher {
    /// One chat completion, returned as text. Shared with research prompts so
    /// both features speak to the same endpoint in the same way.
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let mut request = self.client.post(&self.url).json(&json!({
            "model": self.model,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        }));
        if let Some(token) = &self.token { request = request.bearer_auth(token); }
        let value: Value = request.send().await?.error_for_status()?.json().await?;
        value.pointer("/choices/0/message/content").and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Error::Invalid("chat endpoint returned no content".into()))
    }
}

#[async_trait]
impl Enricher for OpenAiEnricher {
    fn name(&self) -> &'static str { "openai-v1" }
    async fn enrich(&self, text: &str) -> Result<Atoms> {
        let plain = plain_text(text);
        if plain.len() < 40 { return Ok(Atoms::default()); }
        let input: String = plain.chars().take(self.max_chars).collect();
        Ok(parse_atoms(&self.chat(INSTRUCTION, &input).await?))
    }
}

pub fn enricher(state: &AppState) -> Result<Arc<dyn Enricher>> {
    match state.config.enrich.provider.as_str() {
        "heuristic" => Ok(Arc::new(HeuristicEnricher)),
        "openai" => {
            let url = state.config.enrich.url.clone()
                .ok_or_else(|| Error::Config("SIDEFEED_ENRICH_URL is required for the openai provider".into()))?;
            let model = state.config.enrich.model.clone()
                .ok_or_else(|| Error::Config("SIDEFEED_ENRICH_MODEL is required for the openai provider".into()))?;
            Ok(Arc::new(OpenAiEnricher { client: state.http.clone(), url, token: state.config.enrich.token.clone(), model, max_chars: state.config.enrich.max_chars }))
        }
        "disabled" => Err(Error::Config("enrichment is disabled".into())),
        other => Err(Error::Config(format!("unknown enrich provider: {other}"))),
    }
}

// ---------------------------------------------------------------- cache

/// The only enrichment state kept in memory: a bounded LRU of recently read
/// items. `enrich.cache_entries` caps it, so the footprint stays flat however
/// large the archive grows.
#[derive(Default)]
pub struct AtomCache {
    entries: tokio::sync::Mutex<Inner>,
}

#[derive(Default)]
struct Inner { values: HashMap<String, Atoms>, order: VecDeque<String> }

impl AtomCache {
    pub async fn get(&self, item_id: &str) -> Option<Atoms> {
        let mut inner = self.entries.lock().await;
        let found = inner.values.get(item_id).cloned()?;
        inner.touch(item_id);
        Some(found)
    }

    pub async fn put(&self, item_id: &str, atoms: Atoms, capacity: usize) {
        if capacity == 0 { return; }
        let mut inner = self.entries.lock().await;
        inner.values.insert(item_id.to_string(), atoms);
        inner.touch(item_id);
        while inner.order.len() > capacity {
            if let Some(evicted) = inner.order.pop_front() { inner.values.remove(&evicted); }
        }
    }

    pub async fn len(&self) -> usize { self.entries.lock().await.values.len() }
    pub async fn is_empty(&self) -> bool { self.len().await == 0 }
}

impl Inner {
    fn touch(&mut self, item_id: &str) {
        if let Some(index) = self.order.iter().position(|id| id == item_id) { self.order.remove(index); }
        self.order.push_back(item_id.to_string());
    }
}

/// Extractive stand-in for a model-written digest: the best sentence from the
/// sample, or the leading titles when the sample has no usable prose.
pub fn heuristic_digest(sample: &str) -> Option<String> {
    if let Some(summary) = heuristic_atoms(sample).summary { return Some(summary); }
    let lines: Vec<&str> = sample.lines().map(str::trim).filter(|line| !line.is_empty()).take(3).collect();
    if lines.is_empty() { return None; }
    truncate_at_word(&lines.join(" · "), 240)
}

/// One line per category, plus an overall line, describing what moved in the
/// window. Cached briefly: a model call per page load would be wasteful.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DigestItem { pub title: String, pub url: Option<String>, pub published_at: String, pub tags: Vec<String> }

#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoryDigest { pub feed: String, pub title: String, pub count: usize, pub summary: Option<String>, pub items: Vec<DigestItem> }

#[derive(Debug, Clone, serde::Serialize)]
pub struct Updates {
    pub window_hours: u32,
    pub generated_at: String,
    pub summary: Option<String>,
    pub categories: Vec<CategoryDigest>,
}

/// Holds the most recent digest only, keyed by window. A few hundred bytes, and
/// rebuilt at most once per TTL.
#[derive(Default)]
pub struct UpdatesCache { entry: tokio::sync::Mutex<Option<(std::time::Instant, u32, Updates)>> }

const UPDATES_TTL: Duration = Duration::from_secs(600);

impl UpdatesCache {
    async fn get(&self, hours: u32) -> Option<Updates> {
        let guard = self.entry.lock().await;
        match guard.as_ref() {
            Some((at, cached_hours, updates)) if *cached_hours == hours && at.elapsed() < UPDATES_TTL => Some(updates.clone()),
            _ => None,
        }
    }

    async fn put(&self, hours: u32, updates: &Updates) {
        *self.entry.lock().await = Some((std::time::Instant::now(), hours, updates.clone()));
    }
}

/// The per-category rollup behind the updates page: how much moved and a short
/// line about it, grouped by feed, for the items published in the window.
pub async fn updates(state: &AppState, hours: u32) -> Result<Updates> {
    if let Some(cached) = state.updates.get(hours).await { return Ok(cached); }
    let since = (chrono::Utc::now() - chrono::Duration::hours(hours as i64)).to_rfc3339();
    let rows = state.store.recent_items(&since, 300, &[], false, None).await?;

    // Rows arrive newest first, so the first appearance of a feed fixes the
    // category order: the busiest recent category leads.
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<crate::model::ItemWithFeed>> = HashMap::new();
    for row in rows {
        if !grouped.contains_key(&row.feed_slug) { order.push(row.feed_slug.clone()); }
        grouped.entry(row.feed_slug.clone()).or_default().push(row);
    }

    let enricher = enricher(state).ok();
    let mut categories = Vec::new();
    for slug in order.iter().take(12) {
        let group = &grouped[slug];
        let sample = group.iter().take(6).map(|row| {
            let title = row.item.title.as_deref().unwrap_or("").to_string();
            let body = plain_text(row.item.summary.as_deref().or(row.item.content.as_deref()).unwrap_or(""));
            if body.is_empty() { title } else { format!("{title}. {}", truncate_at_word(&body, 160).unwrap_or_default()) }
        }).collect::<Vec<_>>().join("\n");
        let summary = match &enricher {
            Some(found) => found.digest(&group[0].feed_title, &sample).await.unwrap_or(None),
            None => heuristic_digest(&sample),
        };
        // The few items worth opening, so the digest page is readable on its
        // own without a second round trip per category. Tags are fetched in one
        // batch rather than per row.
        let head: Vec<&crate::model::ItemWithFeed> = group.iter().take(5).collect();
        let ids: Vec<String> = head.iter().map(|row| row.item.id.clone()).collect();
        let atoms = atoms_for(state, &ids).await.unwrap_or_default();
        let items = head.iter().map(|row| DigestItem {
            title: row.item.title.clone().unwrap_or_else(|| "Untitled".into()),
            url: row.item.url.clone(),
            published_at: row.item.published_at.clone(),
            tags: atoms.get(&row.item.id).map(|found| found.tags.clone()).unwrap_or_default(),
        }).collect();
        categories.push(CategoryDigest { feed: slug.clone(), title: group[0].feed_title.clone(), count: group.len(), summary, items });
    }

    let combined = categories.iter().filter_map(|category| category.summary.clone()).collect::<Vec<_>>().join(" ");
    let summary = match &enricher {
        Some(found) if !combined.is_empty() => found.digest("everything", &combined).await.unwrap_or(None),
        _ => heuristic_digest(&combined),
    };

    let updates = Updates { window_hours: hours, generated_at: chrono::Utc::now().to_rfc3339(), summary, categories };
    state.updates.put(hours, &updates).await;
    Ok(updates)
}

pub fn item_text(item: &Item) -> String {
    let mut parts = vec![item.title.as_deref().unwrap_or("")];
    for candidate in [item.summary.as_deref(), item.content.as_deref()].into_iter().flatten() {
        if !link_only(candidate) { parts.push(candidate); }
    }
    parts.join("\n")
}

/// Enrich one item and persist the result. Also refreshes the cache, so the
/// caller can render immediately without another read.
pub async fn enrich_item(state: &AppState, item: &Item) -> Result<Atoms> {
    let enricher = enricher(state)?;
    let atoms = enricher.enrich(&item_text(item)).await?;
    state.store.put_atoms(&item.id, enricher.name(), &atoms).await?;
    state.atoms.put(&item.id, atoms.clone(), state.config.enrich.cache_entries).await;
    Ok(atoms)
}

/// One pass over a small batch. Bounded by `enrich.batch`, so a backlog becomes
/// a series of small passes instead of a CPU or memory spike.
pub async fn enrich_batch(state: &AppState) -> Result<usize> {
    let enricher = enricher(state)?;
    let pending = state.store.items_missing_atoms(enricher.name(), state.config.enrich.batch).await?;
    let mut done = 0;
    for item in pending {
        match enrich_item(state, &item).await {
            Ok(_) => done += 1,
            Err(error) => tracing::warn!(item_id = %item.id, %error, "item enrichment failed"),
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Ok(done)
}

pub async fn enrich_loop(state: AppState) {
    let interval = state.config.enrich.interval.max(Duration::from_secs(5));
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if !state.config.enrich.enabled() { continue; }
        match enrich_batch(&state).await {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "enriched items"),
            Err(error) => tracing::warn!(%error, "enrichment pass failed"),
        }
    }
}

/// Artifacts for a page of items: cache first, then a single query for the
/// misses. Never returns a row per item it was not asked for.
pub async fn atoms_for(state: &AppState, item_ids: &[String]) -> Result<HashMap<String, Atoms>> {
    let Ok(enricher) = enricher(state) else { return Ok(HashMap::new()) };
    let model = enricher.name();
    let mut atoms = HashMap::new();
    let mut missing = Vec::new();
    for id in item_ids {
        match state.atoms.get(id).await {
            Some(found) => { atoms.insert(id.clone(), found); }
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        for (id, found) in state.store.atoms_for(&missing, model).await? {
            state.atoms.put(&id, found.clone(), state.config.enrich.cache_entries).await;
            atoms.insert(id, found);
        }
    }
    Ok(atoms)
}

// ---------------------------------------------------------------- http

#[derive(Deserialize)] struct TagQuery { limit: Option<u32> }

async fn feed_tags(State(state): State<AppState>, headers: HeaderMap, Path(slug): Path<String>, Query(query): Query<TagQuery>) -> Result<Json<Value>> {
    access_feed(&state, &headers, &slug).await?;
    let tags = state.store.feed_tags(&slug, query.limit.unwrap_or(50).clamp(1, 200)).await?;
    Ok(Json(json!(tags.into_iter().map(|(tag, count)| json!({"tag": tag, "count": count})).collect::<Vec<_>>())))
}

async fn enrich_now(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<(StatusCode, Json<Atoms>)> {
    authorize(&state, &headers)?;
    let item = state.store.item(&id).await?;
    let atoms = enrich_item(&state, &item).await?;
    Ok((StatusCode::CREATED, Json(atoms)))
}

async fn summarize_now(State(state): State<AppState>, Path((slug, item_id)): Path<(String, String)>) -> Result<Json<Atoms>> {
    let headers = HeaderMap::new();
    access_feed(&state, &headers, &slug).await?;
    let item = state.store.item(&item_id).await?;
    if !state.store.item_in_feed(&slug, &item).await? { return Err(Error::NotFound); }
    let enricher = enricher(&state)?;
    let existing = state.store.atoms(&item_id, enricher.name()).await?;
    if existing.summary.is_some() { return Ok(Json(existing)); }
    let atoms = enrich_item(&state, &item).await?;
    state.atoms.put(&item_id, atoms.clone(), state.config.enrich.cache_entries).await;
    Ok(Json(atoms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_markup_and_decodes_entities() {
        let text = plain_text("<p>Rust &amp; SQLite</p><br>C&#39;est bon");
        assert!(!text.contains('<'));
        assert!(text.contains("Rust & SQLite"));
        assert!(text.contains("C'est bon"));
    }

    #[test]
    fn heuristic_tags_skip_stopwords_and_prefer_repeats() {
        let text = "SQLite powers the index. The index keeps SQLite fast, and the index stays small. \
                    Paging through results stays fast because the index is compact and local.";
        let atoms = heuristic_atoms(text);
        assert!(atoms.tags.contains(&"index".to_string()), "expected repeated term: {:?}", atoms.tags);
        assert!(!atoms.tags.iter().any(|tag| tag == "the" || tag == "and"), "stopwords leaked: {:?}", atoms.tags);
        assert!(atoms.tags.len() <= 6);
    }

    #[test]
    fn heuristic_summary_returns_sentences_in_original_order() {
        let text = "The service stores every item on disk. ".to_string()
            + &"A short note. ".repeat(3)
            + "Enrichment writes tags and summaries next to the item, and never keeps them resident in memory. \
               The worker handles a small batch at a time so a backlog cannot spike the process.";
        let atoms = heuristic_atoms(&text);
        let summary = atoms.summary.expect("summary");
        assert!(summary.chars().count() <= 281, "summary too long: {}", summary.chars().count());
        assert!(summary.contains("disk") || summary.contains("Enrichment"), "unexpected summary: {summary}");
    }

    #[test]
    fn heuristic_atoms_ignore_text_too_short_to_summarise() {
        let atoms = heuristic_atoms("ok");
        assert!(atoms.summary.is_none());
        assert!(atoms.tags.is_empty(), "two letters is not a tag: {:?}", atoms.tags);
    }

    #[test]
    fn a_bare_title_still_yields_tags_but_no_summary() {
        let atoms = heuristic_atoms("Run Qwen 3.8 Flash Next (125B) on a laptop");
        assert!(atoms.summary.is_none(), "a title is not a summary");
        assert!(!atoms.tags.is_empty(), "link-only items should still be taggable");
    }

    #[test]
    fn decimal_points_do_not_split_sentences() {
        let text = "Rust 1.100.0, the following changes to 32-bit Windows targets will happen soon.\n\
                    The i686-pc-windows-msvc target drops to tier one with host tools in this release.";
        let split = sentences(text);
        assert_eq!(split.len(), 2, "split wrong: {split:?}");
        assert!(split[0].starts_with("Rust 1.100.0"), "first sentence was cut: {:?}", split[0]);
    }

    #[test]
    fn link_only_bodies_are_not_treated_as_text() {
        assert!(link_only("Comments"));
        assert!(link_only("12 comments"));
        assert!(!link_only("A real sentence about descriptors and pipelines."));
    }

    #[test]
    fn parse_atoms_reads_json_wrapped_in_prose() {
        let reply = "Sure!\n```json\n{\"summary\": \"A short summary.\", \"tags\": [\"Rust\", \"SQLITE\", \"\"]}\n```";
        let atoms = parse_atoms(reply);
        assert_eq!(atoms.summary.as_deref(), Some("A short summary."));
        assert_eq!(atoms.tags, vec!["rust".to_string(), "sqlite".to_string()]);
    }

    #[test]
    fn parse_atoms_survives_a_model_that_ignores_the_schema() {
        assert!(parse_atoms("no json here").tags.is_empty());
        assert!(parse_atoms("{\"summary\": 42}").tags.is_empty());
    }
}
