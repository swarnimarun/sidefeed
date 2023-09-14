use error_stack::{Result, ResultExt};
use futures_util::{stream::BoxStream, StreamExt, TryStreamExt};
use sqlx::{sqlite::*, Acquire};

use crate::errors::ApplicationError;

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

    /// add url to urls table and update it in the feeds table
    ///
    /// take care to not add duplicate URLs with just slightly different strings,
    /// consider handling url simplification before pushing to this DB.
    ///
    /// note: feeds table holds the meta data for all urls, relevant for building a feed
    /// this allows for simpler normalization constraints as we grow the application complexity
    pub async fn add_feed_source(&self, url: String) -> Result<(), ApplicationError> {
        let mut pool_conn = self
            .0
            .acquire()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;
        let mut tx = pool_conn
            .begin()
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;

        _ = sqlx::query!("INSERT INTO urls VALUES (?, ?)", url, "rss")
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
