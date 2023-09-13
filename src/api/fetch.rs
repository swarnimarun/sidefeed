use actix_web::{web::Json, HttpResponse, Responder};
use serde::Deserialize;

// #[derive(Deserialize)]
// pub struct Req {
//     name: String,
// }

#[actix_web::get("/feed")]
pub async fn get_feed() -> impl Responder {
    Json("hello, world!")
}
