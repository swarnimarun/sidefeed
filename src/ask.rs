//! Feed-scoped Q&A: hybrid lexical retrieval (plus a vector re-rank when an
//! embedding provider is live) answered by the configured chat model, with an
//! extractive fallback that quotes the index when no model is configured.
//!
//! `POST /api/v1/feeds/{slug}/ask {q} -> {answer, citations}`. The question is
//! capped at 500 chars and the route carries its own strict limiter (burst 5),
//! so model-backed answers cannot be machine-gunned.

use std::collections::HashMap;
use axum::{extract::{Path, State}, http::HeaderMap, routing::post, Json, Router};
use serde_json::{json, Value};
use crate::{error::{Error, Result}, model::Item, ratelimit::RateLimitLayer, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/feeds/{slug}/ask", post(ask))
        .layer(RateLimitLayer::ask())
}

#[derive(serde::Deserialize)]
struct AskInput {
    q: String,
}

async fn ask(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(input): Json<AskInput>,
) -> Result<Json<Value>> {
    let feed = crate::api::access_feed(&state, &headers, &slug).await?;
    let q: String = input.q.trim().chars().take(500).collect();
    if q.is_empty() {
        return Err(Error::Invalid("q is required".into()));
    }
    // 1. Lexical top-8 via the existing FTS path (each term quoted and
    // prefix-matched, mirroring `api::fts_query`, so punctuation in a question
    // cannot turn into a syntax error).
    let hits = state.store.search(&slug, &fts_quote(&q), 8).await.unwrap_or_default();
    // 2. Optional vector re-rank when an embedding provider is live.
    let ranked = rerank(&state, &q, hits).await;
    // 3. Chat model when one is configured, else quote the best hit. The
    // capability check (does this enricher answer?) replaces a provider-name
    // comparison, so a future local chat provider just works.
    let answer = match crate::enrich::enricher(&state) {
        Ok(found) => {
            let excerpts = excerpt_text(&ranked);
            match found.answer(&feed.title, &q, &excerpts).await.unwrap_or(None) {
                Some(text) if !text.trim().is_empty() => text,
                _ => extractive_answer(&q, &ranked),
            }
        }
        Err(_) => extractive_answer(&q, &ranked),
    };
    Ok(Json(json!({
        "answer": answer,
        "citations": ranked.iter().take(3).map(|item| json!({
            "item_id": item.id,
            "title": item.title,
            "url": item.url,
        })).collect::<Vec<_>>(),
    })))
}

/// User text becomes an FTS5 expression, the same quoting `api.rs` uses.
fn fts_quote(raw: &str) -> String {
    raw.split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|character| character.is_alphanumeric() || *character == '-' || *character == '_')
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let (an, bn) = (
        a.iter().map(|v| v * v).sum::<f32>().sqrt(),
        b.iter().map(|v| v * v).sum::<f32>().sqrt(),
    );
    if an == 0.0 || bn == 0.0 {
        0.0
    } else {
        dot / (an * bn)
    }
}

/// Re-rank lexical hits by cosine to the question when vectors exist. Anything
/// without a stored vector keeps its lexical position at the tail, and when no
/// provider is live the order is untouched. Only the hit ids are fetched, in
/// one query, so a question never scans the whole embeddings table.
async fn rerank(state: &AppState, q: &str, hits: Vec<Item>) -> Vec<Item> {
    let Ok(provider) = crate::ai::provider(state) else {
        return hits;
    };
    let Ok(needle) = provider.embed(q).await else {
        return hits;
    };
    let ids: Vec<String> = hits.iter().map(|item| item.id.clone()).collect();
    let Ok(vectors) = state.store.embeddings_for(&ids, provider.name()).await else {
        return hits;
    };
    let by_id: HashMap<&str, &Vec<f32>> =
        vectors.iter().map(|(id, vector)| (id.as_str(), vector)).collect();
    let mut scored: Vec<(f32, usize, Item)> = hits
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let score = by_id
                .get(item.id.as_str())
                .filter(|vector| vector.len() == needle.len())
                .map(|vector| cosine(&needle, vector))
                .unwrap_or(f32::NEG_INFINITY);
            (score, index, item)
        })
        .collect();
    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    scored.into_iter().map(|(_, _, item)| item).collect()
}

/// The text a chat model may quote: the top hits as title plus plain prose.
fn excerpt_text(ranked: &[Item]) -> String {
    ranked
        .iter()
        .take(3)
        .map(|item| {
            let title = item.title.as_deref().unwrap_or("Untitled");
            let body = crate::enrich::plain_text(
                item.summary.as_deref().or(item.content.as_deref()).unwrap_or(""),
            );
            let clipped: String = body.chars().take(800).collect();
            format!("Title: {title}\n{clipped}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Quote the index when no chat model is configured: the two query-best
/// sentences of the top hit, prefixed with their source so the reply is never
/// mistaken for generation.
fn extractive_answer(q: &str, ranked: &[Item]) -> String {
    let Some(best) = ranked.first() else {
        return "No matching items in this feed.".to_string();
    };
    let title = best.title.as_deref().unwrap_or("Untitled");
    let body = crate::enrich::plain_text(
        best.summary.as_deref().or(best.content.as_deref()).unwrap_or(""),
    );
    let query_terms: Vec<String> = q_terms(q);
    let mut scored: Vec<(usize, usize)> = crate::enrich::sentences(&body)
        .iter()
        .enumerate()
        .map(|(index, sentence)| {
            let terms = q_terms(sentence);
            let overlap = terms
                .iter()
                .filter(|term| query_terms.contains(term))
                .count();
            (overlap, index)
        })
        .collect();
    scored.sort_by(|left, right| {
        right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1))
    });
    let mut picked: Vec<usize> = scored
        .iter()
        .take(2)
        .filter(|(overlap, _)| *overlap > 0)
        .map(|(_, index)| *index)
        .collect();
    if picked.is_empty() {
        // Nothing shares a word with the title; the lede still answers "what".
        picked = scored.iter().take(1).map(|(_, index)| *index).collect();
    }
    picked.sort_unstable();
    let sentences = crate::enrich::sentences(&body);
    let quote = picked
        .iter()
        .filter_map(|index| sentences.get(*index).copied())
        .collect::<Vec<_>>()
        .join(" ");
    if quote.is_empty() {
        return format!("from {title}: no quotable text in the top hit.");
    }
    format!("from {title}: {quote}")
}

fn q_terms(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .map(|word| word.to_lowercase())
        .filter(|word| word.len() >= 3)
        .collect()
}
