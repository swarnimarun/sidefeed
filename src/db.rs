use error_stack::{Result, ResultExt};
use futures_util::{stream::BoxStream, StreamExt, TryStreamExt};
use sqlx::{sqlite::*, Acquire};

use crate::{errors::ApplicationError, api::update::FeedEdit};

pub struct AppDB(SqlitePool);

impl AppDB {
    /// build a connection pool with max connections == 4
    pub async fn try_build_pool(url: &str) -> Result<Self, ApplicationError> {
        // assume higher reads fewer writes.
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(url)
            .await
            .change_context(ApplicationError::DatabaseConnectionFailed)?;
        // ensure migrations are applied before usage
        sqlx::migrate!()
            .run(&pool)
            .await
            .change_context(ApplicationError::DatabaseMigrationsFailed)?;
        Ok(AppDB(pool))
    }

    /// get stream of output from db feeds table
    /// works better with any streaming apis, even
    /// for non streaming apis consider using this
    /// and chunking outputs to increase responsiveness
    /// and maximum memory usage.
    pub fn feed_stream<'e>(
        &'e self,
    ) -> BoxStream<'e, std::result::Result<(i64, String, String, String), sqlx::Error>> {
        // TODO(swarnim): don't query all columns if you don't need them, works for now though
        sqlx::query!("SELECT * FROM feeds")
            .fetch(&self.0)
            .map_ok(|rec| (rec.id, rec.url, rec.last_checked, rec.last_modified))
            .boxed()
    }

    /// Edit the feed url specified by the associated ID.
    /// The old url will be deleted after the feed is updated.
    /// 
    /// TODO(rs): Is this worth having as an upsert?
    pub async fn edit_feed_source(&self, edit: FeedEdit) -> Result<(), ApplicationError> {
        let mut pool_conn = self
            .0
            .acquire()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        // Just to be safe.
        let mut tx = pool_conn
            .begin()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        let old_url = sqlx::query!("SELECT url FROM feeds WHERE id = ?", edit.id)
            .fetch_optional(tx.as_mut())
            .await
            .change_context(ApplicationError::DatabaseQueryError)?.map(|it| it.url);
 
        if old_url.is_none() {
            return Err(ApplicationError::UnexpectedError("No matching feed for id found.").into());
        }

        let datetime = sqlx::types::time::OffsetDateTime::now_utc().to_string();

        _ = sqlx::query!("INSERT INTO urls VALUES (?, ?)", edit.new_url.url, "rss")
            .execute(tx.as_mut())
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        _ = sqlx::query!(
            "UPDATE feeds SET url = ?, last_checked = ?, last_modified = ? WHERE id = ?",
            edit.new_url.url,
            datetime,
            datetime,
            edit.id,
        )
        .execute(tx.as_mut())
        .await
        .change_context(ApplicationError::DatabaseQueryError)?;

        // we remove it after we change the url, so the delete doesn't cascade.
        if let Some(old_url) = old_url {
            _ = self.remove_feed_source(old_url).await?;
        }

        tx.commit()
            .await
            .change_context(ApplicationError::DatabaseQueryError)
    }

    /// Add url to urls table and update it in the feeds table
    ///
    /// Take care to not add duplicate URLs with just slightly different strings,
    /// consider handling url simplification before pushing to this DB.
    ///
    /// Note: the feeds table holds the meta data for all urls, relevant for building a feed.
    /// This allows for simpler normalization constraints as we grow the application complexity
    /// 
    pub async fn add_feed_source(&self, url: String, feed_type: String) -> Result<(), ApplicationError> {
        let mut pool_conn = self
            .0
            .acquire()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;
        let mut tx = pool_conn
            .begin()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        _ = sqlx::query!("INSERT INTO urls VALUES (?, ?)", url, feed_type)
            .execute(tx.as_mut())
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        let datetime = sqlx::types::time::OffsetDateTime::now_utc().to_string();
        _ = sqlx::query!(
            "INSERT INTO feeds (url, last_checked, last_modified) VALUES (?, ?, ?)",
            url,
            datetime,
            datetime
        )
        .execute(tx.as_mut())
        .await
        .change_context(ApplicationError::DatabaseQueryError)?;

        tx.commit()
            .await
            .change_context(ApplicationError::DatabaseQueryError)
    }

    /// remove url from urls table and from feeds table due to cascade delete
    pub async fn remove_feed_source(&self, url: String) -> Result<(), ApplicationError> {
        let _ = sqlx::query!("DELETE FROM urls WHERE url = ?", url)
            .execute(&self.0)
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;
        Ok(())
    }
}
