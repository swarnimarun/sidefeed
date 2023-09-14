use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};

use crate::db::AppDB;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Url {
    url: String,
}

#[actix_web::post("/feed/add")]
pub async fn add_feed_source(data: web::Data<Arc<AppDB>>, body: web::Json<Url>) -> impl Responder {
    let db = data.into_inner();
    // TODO(swarnim): validate before adding to db
    _ = db.add_feed_source(body.into_inner().url).await;
    HttpResponse::Ok()
}

#[actix_web::post("/feed/remove")]
pub async fn remove_feed_source(
    data: web::Data<Arc<AppDB>>,
    body: web::Json<Url>,
) -> impl Responder {
    let db = data.into_inner();
    _ = db.remove_feed_source(body.into_inner().url).await;
    HttpResponse::Ok()
}

#[actix_web::post("/feed/edit")]
pub async fn edit_feed_source() -> impl Responder {
    // TODO(swarnim): implement correctly
    HttpResponse::Ok()
}
