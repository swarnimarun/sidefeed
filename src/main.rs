mod api;
mod db;
mod errors;
mod models;

use env_struct::env_struct;
use error_stack::{Report, Result, ResultExt};
use pretty_env_logger::formatted_builder;

use actix_web::{
    middleware::Logger,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};

use crate::{
    api::{
        fetch::get_feed_whole,
        update::{add_feed_source, edit_feed_source, remove_feed_source},
    },
    errors::ApplicationError,
};

env_struct! {
    struct ApplicationEnv {
        log = "true".into(),
    }
}

/// handle all unknown requests for now, NOT_FOUND(404) for all,
async fn handle_unknown() -> impl Responder {
    // TODO(swarnim):
    // consider providing suggestions for miss-spellings eg, /feeds vs /feed
    HttpResponse::NotFound()
}

#[actix_web::main]
async fn main() -> Result<(), ApplicationError> {
    // setup env and logging
    let env = ApplicationEnv::load_from_env();
    if matches!(env.log.to_lowercase().as_str(), "true" | "yes" | "on") {
        formatted_builder()
            // TODO(swarnim): before deploying consider setting this to OFF
            // users can manually set LOG=off for the time being
            .filter_level(log::LevelFilter::Debug)
            .parse_default_env()
            .try_init()
            .change_context(ApplicationError::UnexpectedError(
                "Intialized logging twice.",
            ))?;
    }

    // setup db and wrap in shareable handler Arc
    let db = std::sync::Arc::new(db::AppDB::try_build_pool("sqlite://db/sqlite.db").await?);

    // startup the server with the actix app
    const ADDR: (&str, u16) = ("127.0.0.1", 3000);
    println!("Listening on http://{}:{}", ADDR.0, ADDR.1);
    HttpServer::new(move || {
        App::new()
            // send shared sqlx db connection pool
            // TODO(swarnim): consider providing tracing middleware
            .app_data(web::Data::new(db.clone()))
            .wrap(Logger::default())
            .service(get_feed_whole)
            .service(add_feed_source)
            .service(remove_feed_source)
            .service(edit_feed_source)
            .default_service(web::to(handle_unknown))
    })
    .bind(ADDR)
    .change_context(ApplicationError::ServerFailure)
    .attach_printable_lazy(|| format!("Trying to bind -> {ADDR:?}"))?
    .run()
    .await
    .change_context(ApplicationError::ServerFailure)
}
