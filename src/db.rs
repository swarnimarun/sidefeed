use error_stack::{Report, Result, ResultExt};
use sqlx::sqlite::*;

use crate::errors::ApplicationError;

pub struct AppDB(SqlitePool);

impl AppDB {
    async fn try_build_pool(url: &str) -> Result<Self, ApplicationError> {
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

    async fn build_feed(&self) -> Result<crate::models::Feed, ApplicationError> {
        // TODO(swarnim): build the actual field
        Ok(crate::models::Feed::new())
    }
    async fn add_feed_source(&self, url: String) -> Result<(), ApplicationError> {
        let f = sqlx::query!("INSERT INTO urls VALUES (?, ?)", url, "rss")
            .execute(&self.0)
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;
        if f.rows_affected() == 0 {
            return Err(ApplicationError::DatabaseQueryFailed).map_err(Report::from);
        }
        let date_time = sqlx::types::time::OffsetDateTime::now_utc();
        let dt_str = date_time.to_string();
        let g = sqlx::query!(
            "INSERT INTO feeds (url, last_checked, last_modified) VALUES (?, ?, ?)",
            url,
            dt_str,
            dt_str
        )
        .execute(&self.0)
        .await
        .change_context(ApplicationError::DatabaseQueryError)?;
        if g.rows_affected() == 0 {
            return Err(ApplicationError::DatabaseQueryFailed).map_err(Report::from);
        }
        Ok(())
    }
    async fn remove_feed_source(&self, url: String) -> Result<(), ApplicationError> {
        let f = sqlx::query!("DELETE FROM urls WHERE url = ?", url)
            .execute(&self.0)
            .await
            .change_context(ApplicationError::DatabaseQueryError)?;
        if f.rows_affected() == 0 {
            return Err(ApplicationError::DatabaseQueryFailed).map_err(Report::from);
        }
        Ok(())
    }
}
