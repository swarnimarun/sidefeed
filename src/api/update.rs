use actix_web::{HttpResponse, Responder};

#[actix_web::post("/feed/add")]
pub async fn add_feed_source() -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::post("/feed/remove")]
pub async fn remove_feed_source() -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::post("/feed/edit")]
pub async fn edit_feed_source() -> impl Responder {
    HttpResponse::Ok()
}
