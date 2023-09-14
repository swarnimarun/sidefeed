use actix_web::{
    web::{self, Bytes, Json},
    HttpRequest, HttpResponse, Responder,
};
use bytes::Buf;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::db::AppDB;

#[derive(Deserialize, Serialize)]
pub struct Post {
    title: String,
    url: String,
    summary: String,
}

#[actix_web::get("/feed")]
pub async fn get_feed_whole(data: web::Data<std::sync::Arc<AppDB>>) -> impl Responder {
    let db = data.into_inner();
    let mut ft = db.feed_stream();
    let mut posts = vec![];
    let client = awc::Client::default();
    while let Ok(Some((_, url, _, _))) = ft.try_next().await {
        // make a request to the blog check latest and return
        if let Ok(mut res) = client.get(&url).send().await {
            if res.status().is_success() {
                // let src = .await.unwrap_or(Bytes::default());
                let rss_chan =
                    rss::Channel::read_from(res.body().await.unwrap_or_default().reader())
                        .unwrap_or_default();
                for post in rss_chan.into_items() {
                    let get_post_info = || Some((post.title?, post.link?, post.description?));
                    if let Some((title, url, summary)) = get_post_info() {
                        posts.push(Post {
                            title,
                            url,
                            summary,
                        });
                    }
                }
            }
        }
    }
    Json(posts)
}
