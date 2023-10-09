use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};

use crate::{db::AppDB, models::FeedSourceId};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UrlInput {
    pub url: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FeedEdit {
    pub id: FeedSourceId,
    pub new_url: UrlInput,
}

#[actix_web::post("/feed/add")]
pub async fn add_feed_source(data: web::Data<Arc<AppDB>>, body: web::Json<UrlInput>) -> impl Responder {
    let db = data.into_inner();
    // TODO(swarnim): validate before adding to db
    _ = db.add_feed_source(body.into_inner().url, "rss".into()).await;
    HttpResponse::Ok()
}

#[actix_web::post("/feed/remove")]
pub async fn remove_feed_source(
    data: web::Data<Arc<AppDB>>,
    body: web::Json<UrlInput>,
) -> impl Responder {
    let db = data.into_inner();
    _ = db.remove_feed_source(body.into_inner().url).await;
    HttpResponse::Ok()
}

#[actix_web::post("/feed/edit")]
pub async fn edit_feed_source(
    data: web::Data<Arc<AppDB>>,
    body: web::Json<FeedEdit>,
) -> impl Responder {
    let db = data.into_inner();
    _ = db.edit_feed_source(body.into_inner()).await;
    HttpResponse::Ok()
}
