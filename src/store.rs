use chrono::Utc;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::{collections::HashMap, str::FromStr};
use uuid::Uuid;
use crate::{error::{Error, Result}, model::{Atoms, Feed, Item, ItemWithFeed, NewItem, Peer, Source}};

#[derive(Clone)]
pub struct Store { pool: SqlitePool }

/// Options for a full-text search, grouped so call sites read as named fields
/// rather than a long positional tail.
pub struct SearchOptions<'a> {
    pub query: &'a str,
    pub since: &'a str,
    pub limit: u32,
    pub feed: Option<&'a str>,
    pub tags: &'a [String],
    pub all_tags: bool,
    pub oldest_first: bool,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(|e| Error::Config(format!("invalid database URL: {e}")))?
            .foreign_keys(true).busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().max_connections(8).connect_with(options).await?;
        sqlx::migrate!().run(&pool).await.map_err(|e| Error::Internal(e.to_string()))?;
        sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> Result<()> { sqlx::query("SELECT 1").execute(&self.pool).await?; Ok(()) }

    pub async fn create_source(&self, url: &str, kind: &str, title: Option<&str>) -> Result<Source> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO sources(id,url,kind,title,created_at) VALUES(?,?,?,?,?)")
            .bind(&id).bind(url).bind(kind).bind(title).bind(Utc::now().to_rfc3339())
            .execute(&self.pool).await.map_err(map_unique)?;
        self.source(&id).await
    }

    pub async fn source(&self, id: &str) -> Result<Source> {
        sqlx::query_as("SELECT * FROM sources WHERE id=?").bind(id).fetch_optional(&self.pool).await?.ok_or(Error::NotFound)
    }

    pub async fn sources(&self) -> Result<Vec<Source>> {
        Ok(sqlx::query_as("SELECT * FROM sources ORDER BY created_at DESC").fetch_all(&self.pool).await?)
    }

    pub async fn due_sources(&self, limit: u32) -> Result<Vec<Source>> {
        Ok(sqlx::query_as("SELECT * FROM sources WHERE enabled=1 AND (next_poll_at IS NULL OR next_poll_at<=?) ORDER BY COALESCE(next_poll_at,'') LIMIT ?")
            .bind(Utc::now().to_rfc3339()).bind(limit).fetch_all(&self.pool).await?)
    }

    pub async fn update_source_fetch(&self, id: &str, title: Option<&str>, etag: Option<&str>, modified: Option<&str>, error: Option<&str>, next: &str) -> Result<()> {
        sqlx::query("UPDATE sources SET title=COALESCE(?,title),etag=COALESCE(?,etag),last_modified=COALESCE(?,last_modified),last_error=?,last_polled_at=?,next_poll_at=? WHERE id=?")
            .bind(title).bind(etag).bind(modified).bind(error).bind(Utc::now().to_rfc3339()).bind(next).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn acquire_lease(&self, resource: &str, owner: &str, seconds: i64) -> Result<bool> {
        let now = Utc::now();
        let expires = (now + chrono::Duration::seconds(seconds)).to_rfc3339();
        let result = sqlx::query("INSERT INTO fetch_leases(resource,owner,expires_at) VALUES(?,?,?) ON CONFLICT(resource) DO UPDATE SET owner=excluded.owner,expires_at=excluded.expires_at WHERE fetch_leases.expires_at<?")
            .bind(resource).bind(owner).bind(expires).bind(now.to_rfc3339()).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn release_lease(&self, resource: &str, owner: &str) -> Result<()> {
        sqlx::query("DELETE FROM fetch_leases WHERE resource=? AND owner=?").bind(resource).bind(owner).execute(&self.pool).await?; Ok(())
    }

    pub async fn upsert_item(&self, source_id: Option<&str>, item: &NewItem) -> Result<Item> {
        let stable = item.url.clone().unwrap_or_else(|| format!("{}:{}", source_id.unwrap_or("peer"), item.external_id));
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, stable.as_bytes()).to_string();
        let tags = serde_json::to_string(&item.tags).map_err(|e| Error::Invalid(e.to_string()))?;
        let raw = item.raw.as_ref().map(ToString::to_string);
        sqlx::query("INSERT INTO items(id,source_id,external_id,url,title,summary,content,author,published_at,fetched_at,tags_json,raw_json,visibility) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET url=excluded.url,title=excluded.title,summary=excluded.summary,content=excluded.content,author=excluded.author,published_at=excluded.published_at,fetched_at=excluded.fetched_at,tags_json=excluded.tags_json,raw_json=excluded.raw_json,visibility=excluded.visibility")
            .bind(&id).bind(source_id).bind(&item.external_id).bind(&item.url).bind(&item.title).bind(&item.summary)
            .bind(&item.content).bind(&item.author).bind(&item.published_at).bind(Utc::now().to_rfc3339())
            .bind(tags).bind(raw).bind(&item.visibility).execute(&self.pool).await?;
        self.item(&id).await
    }

    pub async fn item(&self, id: &str) -> Result<Item> {
        sqlx::query_as("SELECT * FROM items WHERE id=?").bind(id).fetch_optional(&self.pool).await?.ok_or(Error::NotFound)
    }

    pub async fn create_feed(&self, slug: &str, title: &str, description: Option<&str>, include: Option<&str>, exclude: Option<&str>, public: bool) -> Result<Feed> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO feeds(id,slug,title,description,include_terms,exclude_terms,public,created_at) VALUES(?,?,?,?,?,?,?,?)")
            .bind(&id).bind(slug).bind(title).bind(description).bind(include).bind(exclude).bind(public).bind(Utc::now().to_rfc3339())
            .execute(&self.pool).await.map_err(map_unique)?;
        self.feed(slug).await
    }

    pub async fn feed(&self, slug: &str) -> Result<Feed> {
        sqlx::query_as("SELECT * FROM feeds WHERE slug=?").bind(slug).fetch_optional(&self.pool).await?.ok_or(Error::NotFound)
    }
    pub async fn feeds(&self) -> Result<Vec<Feed>> { Ok(sqlx::query_as("SELECT * FROM feeds ORDER BY created_at DESC").fetch_all(&self.pool).await?) }

    pub async fn attach_source(&self, slug: &str, source_id: &str) -> Result<()> {
        let feed = self.feed(slug).await?; self.source(source_id).await?;
        sqlx::query("INSERT OR IGNORE INTO feed_sources(feed_id,source_id) VALUES(?,?)").bind(feed.id).bind(source_id).execute(&self.pool).await?; Ok(())
    }

    pub async fn feed_items(&self, slug: &str, limit: u32, cursor: Option<&str>) -> Result<Vec<Item>> {
        self.feed_items_tagged(slug, limit, cursor, &[], false).await
    }

    /// Same listing, optionally narrowed to tags. `all` requires every tag to be
    /// present, otherwise any one of them is enough.
    pub async fn feed_items_tagged(&self, slug: &str, limit: u32, cursor: Option<&str>, tags: &[String], all: bool) -> Result<Vec<Item>> {
        let feed = self.feed(slug).await?;
        let cursor = cursor.unwrap_or("9999-12-31T23:59:59Z");
        let mut sql = String::from("SELECT i.* FROM items i JOIN feed_sources fs ON fs.source_id=i.source_id WHERE fs.feed_id=? AND i.published_at<?");
        append_tag_clause(&mut sql, tags, all);
        sql.push_str(" ORDER BY i.published_at DESC,i.id DESC LIMIT ?");

        let mut query = sqlx::query_as::<_, Item>(&sql).bind(&feed.id).bind(cursor);
        for tag in tags { query = query.bind(tag); }
        if all && !tags.is_empty() { query = query.bind(tags.len() as i64); }
        let mut items: Vec<Item> = query.bind(limit).fetch_all(&self.pool).await?;
        items.retain(|i| matches_filter(&feed, i)); Ok(items)
    }

    // ------------------------------------------------------------ derived artifacts
    // Tags and generated summaries live on disk next to the items they describe.
    // `model` records which enricher produced a row, so changing the model is a
    // new name rather than a migration.

    pub async fn put_atoms(&self, item_id: &str, model: &str, atoms: &Atoms) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        // The summary row doubles as the "processed" marker, so it is written
        // even when the item had nothing worth summarising. Otherwise a
        // link-only item would be reconsidered on every worker pass forever.
        sqlx::query("INSERT INTO item_enrichments(item_id,kind,model,value,created_at) VALUES(?,'summary',?,?,?) ON CONFLICT(item_id,kind,model) DO UPDATE SET value=excluded.value,created_at=excluded.created_at")
            .bind(item_id).bind(model).bind(atoms.summary.as_deref().unwrap_or("")).bind(&now).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM item_tags WHERE item_id=? AND model=?").bind(item_id).bind(model).execute(&mut *tx).await?;
        for tag in atoms.tags.iter().take(12) {
            sqlx::query("INSERT OR IGNORE INTO item_tags(item_id,tag,model,created_at) VALUES(?,?,?,?)")
                .bind(item_id).bind(tag).bind(model).bind(&now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn atoms(&self, item_id: &str, model: &str) -> Result<Atoms> {
        let summary: Option<String> = sqlx::query_scalar("SELECT value FROM item_enrichments WHERE item_id=? AND kind='summary' AND model=?")
            .bind(item_id).bind(model).fetch_optional(&self.pool).await?;
        let tags: Vec<String> = sqlx::query_scalar("SELECT tag FROM item_tags WHERE item_id=? AND model=? ORDER BY tag")
            .bind(item_id).bind(model).fetch_all(&self.pool).await?;
        Ok(Atoms { tags, summary: summary.filter(|text| !text.is_empty()) })
    }

    /// One round trip for a whole page: no per-item query in the render path.
    pub async fn atoms_for(&self, item_ids: &[String], model: &str) -> Result<HashMap<String, Atoms>> {
        let mut result: HashMap<String, Atoms> = item_ids.iter().map(|id| (id.clone(), Atoms::default())).collect();
        if item_ids.is_empty() { return Ok(result); }
        let placeholders = vec!["?"; item_ids.len()].join(",");

        let tag_sql = format!("SELECT item_id,tag FROM item_tags WHERE model=? AND item_id IN ({placeholders}) ORDER BY tag");
        let mut query = sqlx::query_as::<_, (String, String)>(&tag_sql).bind(model);
        for id in item_ids { query = query.bind(id); }
        for (item_id, tag) in query.fetch_all(&self.pool).await? {
            if let Some(atoms) = result.get_mut(&item_id) { atoms.tags.push(tag); }
        }

        let summary_sql = format!("SELECT item_id,value FROM item_enrichments WHERE kind='summary' AND model=? AND item_id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, (String, String)>(&summary_sql).bind(model);
        for id in item_ids { query = query.bind(id); }
        for (item_id, summary) in query.fetch_all(&self.pool).await? {
            if let Some(atoms) = result.get_mut(&item_id) { atoms.summary = Some(summary); }
        }
        Ok(result)
    }

    /// Items this model has not processed yet, newest first. Processing is
    /// marked by the summary row, so an item that yields nothing is not retried.
    pub async fn items_missing_atoms(&self, model: &str, limit: u32) -> Result<Vec<Item>> {
        Ok(sqlx::query_as("SELECT i.* FROM items i WHERE NOT EXISTS(SELECT 1 FROM item_enrichments e WHERE e.item_id=i.id AND e.kind='summary' AND e.model=?) ORDER BY i.published_at DESC LIMIT ?")
            .bind(model).bind(limit).fetch_all(&self.pool).await?)
    }

    /// Tag counts across one feed, most frequent first. Powers the tag list and
    /// the reader's tag filter.
    pub async fn feed_tags(&self, slug: &str, limit: u32) -> Result<Vec<(String, i64)>> {
        Ok(sqlx::query_as("SELECT t.tag, COUNT(*) AS uses FROM item_tags t JOIN items i ON i.id=t.item_id JOIN feed_sources fs ON fs.source_id=i.source_id JOIN feeds f ON f.id=fs.feed_id WHERE f.slug=? GROUP BY t.tag ORDER BY uses DESC, t.tag LIMIT ?")
            .bind(slug).bind(limit).fetch_all(&self.pool).await?)
    }

    /// Public items published inside a window, across every public feed. One row
    /// per item even when several feeds carry it, so the caller can rank across
    /// sources without duplicates.
    pub async fn recent_items(&self, since: &str, limit: u32, tags: &[String], all: bool) -> Result<Vec<ItemWithFeed>> {
        let mut sql = String::from("SELECT i.*, f.slug AS feed_slug, f.title AS feed_title FROM items i JOIN feed_sources fs ON fs.source_id=i.source_id JOIN feeds f ON f.id=fs.feed_id WHERE f.public=1 AND i.visibility='public' AND i.published_at>=?");
        append_tag_clause(&mut sql, tags, all);
        sql.push_str(" GROUP BY i.id ORDER BY i.published_at DESC LIMIT ?");

        let mut query = sqlx::query_as::<_, ItemWithFeed>(&sql).bind(since);
        for tag in tags { query = query.bind(tag); }
        if all && !tags.is_empty() { query = query.bind(tags.len() as i64); }
        Ok(query.bind(limit).fetch_all(&self.pool).await?)
    }

    /// Tag counts across every public feed inside a window. Powers the tag
    /// browser when no single feed is selected.
    pub async fn public_tags(&self, since: &str, limit: u32) -> Result<Vec<(String, i64)>> {
        Ok(sqlx::query_as("SELECT t.tag, COUNT(*) AS uses FROM item_tags t JOIN items i ON i.id=t.item_id JOIN feed_sources fs ON fs.source_id=i.source_id JOIN feeds f ON f.id=fs.feed_id WHERE f.public=1 AND i.published_at>=? GROUP BY t.tag ORDER BY uses DESC, t.tag LIMIT ?")
            .bind(since).bind(limit).fetch_all(&self.pool).await?)
    }

    pub async fn search(&self, slug: &str, query: &str, limit: u32) -> Result<Vec<Item>> {
        let feed = self.feed(slug).await?;
        let mut items: Vec<Item> = sqlx::query_as("SELECT i.* FROM items_fts f JOIN items i ON i.id=f.item_id JOIN feed_sources fs ON fs.source_id=i.source_id WHERE fs.feed_id=? AND items_fts MATCH ? ORDER BY bm25(items_fts),i.published_at DESC LIMIT ?")
            .bind(&feed.id).bind(query).bind(limit).fetch_all(&self.pool).await?;
        items.retain(|i| matches_filter(&feed, i)); Ok(items)
    }

    /// Full-text search across every public feed, optionally narrowed to one
    /// feed, a time window, and a set of tags. Relevance first, then recency;
    /// `oldest_first` flips that for reading a story chronologically.
    ///
    /// FTS5 only allows `bm25()` in a query that scans the index directly, so
    /// ranking happens in an inner scan and the joins run outside it.
    pub async fn search_public(&self, options: SearchOptions<'_>) -> Result<Vec<ItemWithFeed>> {
        let SearchOptions { query, since, limit, feed, tags, all_tags, oldest_first } = options;
        let (inner, rank_bound) = if oldest_first {
            ("SELECT item_id FROM items_fts WHERE items_fts MATCH ? LIMIT 5000".to_string(), None)
        } else {
            let bound = limit.saturating_mul(10).clamp(50, 2000);
            ("SELECT item_id, bm25(items_fts) AS rank FROM items_fts WHERE items_fts MATCH ? ORDER BY rank LIMIT ?".to_string(), Some(bound))
        };
        let mut sql = format!("SELECT i.*, f.slug AS feed_slug, f.title AS feed_title FROM ({inner}) fts JOIN items i ON i.id=fts.item_id JOIN feed_sources fs ON fs.source_id=i.source_id JOIN feeds f ON f.id=fs.feed_id WHERE f.public=1 AND i.published_at>=?");
        if feed.is_some() { sql.push_str(" AND f.slug=?"); }
        append_tag_clause(&mut sql, tags, all_tags);
        sql.push_str(" GROUP BY i.id ORDER BY ");
        sql.push_str(if oldest_first { "i.published_at ASC" } else { "MIN(fts.rank), i.published_at DESC" });
        sql.push_str(" LIMIT ?");

        let mut builder = sqlx::query_as::<_, ItemWithFeed>(&sql).bind(query);
        if let Some(bound) = rank_bound { builder = builder.bind(bound); }
        builder = builder.bind(since);
        if let Some(slug) = feed { builder = builder.bind(slug); }
        for tag in tags { builder = builder.bind(tag); }
        if all_tags && !tags.is_empty() { builder = builder.bind(tags.len() as i64); }
        Ok(builder.bind(limit).fetch_all(&self.pool).await?)
    }

    pub async fn item_in_feed(&self, slug: &str, item: &Item) -> Result<bool> {
        let feed = self.feed(slug).await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feed_sources WHERE feed_id=? AND source_id=?")
            .bind(&feed.id).bind(&item.source_id).fetch_one(&self.pool).await?;
        Ok(count > 0 && matches_filter(&feed, item))
    }

    pub async fn public_items_since(&self, since: &str, limit: u32) -> Result<Vec<Item>> {
        Ok(sqlx::query_as("SELECT * FROM items WHERE visibility='public' AND fetched_at>? ORDER BY fetched_at ASC LIMIT ?")
            .bind(since).bind(limit).fetch_all(&self.pool).await?)
    }

    pub async fn prune_items(&self, before: &str) -> Result<u64> {
        Ok(sqlx::query("DELETE FROM items WHERE fetched_at<?").bind(before).execute(&self.pool).await?.rows_affected())
    }

    pub async fn create_peer(&self, base_url: &str, secret: &str) -> Result<Peer> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO peers(id,base_url,shared_secret,created_at) VALUES(?,?,?,?)")
            .bind(&id).bind(base_url.trim_end_matches('/')).bind(secret).bind(Utc::now().to_rfc3339()).execute(&self.pool).await.map_err(map_unique)?;
        self.peer(&id).await
    }
    pub async fn peer(&self, id: &str) -> Result<Peer> { sqlx::query_as("SELECT * FROM peers WHERE id=?").bind(id).fetch_optional(&self.pool).await?.ok_or(Error::NotFound) }
    pub async fn peers(&self) -> Result<Vec<Peer>> { Ok(sqlx::query_as("SELECT * FROM peers WHERE enabled=1 ORDER BY created_at").fetch_all(&self.pool).await?) }
    pub async fn touch_peer(&self, id: &str) -> Result<()> { sqlx::query("UPDATE peers SET last_sync_at=? WHERE id=?").bind(Utc::now().to_rfc3339()).bind(id).execute(&self.pool).await?; Ok(()) }

    pub async fn put_embedding(&self, item_id: &str, provider: &str, vector: &[f32]) -> Result<()> {
        let json = serde_json::to_string(vector).map_err(|e| Error::Internal(e.to_string()))?;
        sqlx::query("INSERT INTO embeddings(item_id,provider,dimensions,vector_json,created_at) VALUES(?,?,?,?,?) ON CONFLICT(item_id,provider) DO UPDATE SET dimensions=excluded.dimensions,vector_json=excluded.vector_json,created_at=excluded.created_at")
            .bind(item_id).bind(provider).bind(vector.len() as i64).bind(json).bind(Utc::now().to_rfc3339()).execute(&self.pool).await?; Ok(())
    }
    pub async fn embeddings(&self, provider: &str) -> Result<Vec<(String, Vec<f32>)>> {
        let rows: Vec<(String,String)> = sqlx::query_as("SELECT item_id,vector_json FROM embeddings WHERE provider=?").bind(provider).fetch_all(&self.pool).await?;
        rows.into_iter().map(|(id,json)| serde_json::from_str(&json).map(|v| (id,v)).map_err(|e| Error::Internal(e.to_string()))).collect()
    }
}

/// Restricts a query that aliases `items` as `i` to a set of derived tags. `all`
/// requires every tag; otherwise any one of them is enough. Placeholders are
/// appended in order, so callers must bind the tags (and then the count for
/// `all`) in the same order they appear in `tags`.
fn append_tag_clause(sql: &mut String, tags: &[String], all: bool) {
    if tags.is_empty() { return; }
    let placeholders = vec!["?"; tags.len()].join(",");
    if all {
        sql.push_str(&format!(" AND (SELECT COUNT(DISTINCT t.tag) FROM item_tags t WHERE t.item_id=i.id AND t.tag IN ({placeholders}))=?"));
    } else {
        sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM item_tags t WHERE t.item_id=i.id AND t.tag IN ({placeholders}))"));
    }
}

fn matches_filter(feed: &Feed, item: &Item) -> bool {
    let haystack = format!("{} {} {} {}", item.title.as_deref().unwrap_or(""), item.summary.as_deref().unwrap_or(""), item.content.as_deref().unwrap_or(""), item.tags_json).to_lowercase();
    let terms = |v: &Option<String>| v.as_deref().unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_lowercase).collect::<Vec<_>>();
    let include = terms(&feed.include_terms); let exclude = terms(&feed.exclude_terms);
    (include.is_empty() || include.iter().any(|t| haystack.contains(t))) && !exclude.iter().any(|t| haystack.contains(t))
}
fn map_unique(error: sqlx::Error) -> Error {
    if let sqlx::Error::Database(db) = &error { if db.is_unique_violation() { return Error::Conflict("resource already exists".into()); } }
    Error::Database(error)
}
