use chrono::Utc;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;
use crate::{error::{Error, Result}, model::{Feed, Item, NewItem, Peer, Source}};

#[derive(Clone)]
pub struct Store { pool: SqlitePool }

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
        let feed = self.feed(slug).await?;
        let mut items: Vec<Item> = sqlx::query_as("SELECT i.* FROM items i JOIN feed_sources fs ON fs.source_id=i.source_id WHERE fs.feed_id=? AND i.published_at<? ORDER BY i.published_at DESC,i.id DESC LIMIT ?")
            .bind(feed.id).bind(cursor.unwrap_or("9999-12-31T23:59:59Z")).bind(limit).fetch_all(&self.pool).await?;
        items.retain(|i| matches_filter(&feed, i)); Ok(items)
    }

    pub async fn search(&self, slug: &str, query: &str, limit: u32) -> Result<Vec<Item>> {
        let feed = self.feed(slug).await?;
        let mut items: Vec<Item> = sqlx::query_as("SELECT i.* FROM items_fts f JOIN items i ON i.id=f.item_id JOIN feed_sources fs ON fs.source_id=i.source_id WHERE fs.feed_id=? AND items_fts MATCH ? ORDER BY bm25(items_fts),i.published_at DESC LIMIT ?")
            .bind(feed.id).bind(query).bind(limit).fetch_all(&self.pool).await?;
        items.retain(|i| matches_filter(&feed, i)); Ok(items)
    }

    pub async fn item_in_feed(&self, slug: &str, item: &Item) -> Result<bool> {
        let feed = self.feed(slug).await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feed_sources WHERE feed_id=? AND source_id=?")
            .bind(feed.id).bind(&item.source_id).fetch_one(&self.pool).await?;
        Ok(count > 0 && matches_filter(&feed, item))
    }

    pub async fn public_items_since(&self, since: &str, limit: u32) -> Result<Vec<Item>> {
        Ok(sqlx::query_as("SELECT * FROM items WHERE visibility='public' AND fetched_at>? ORDER BY fetched_at ASC LIMIT ?")
            .bind(since).bind(limit).fetch_all(&self.pool).await?)
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
