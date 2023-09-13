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
        fetch::get_feed,
        update::{add_feed_source, edit_feed_source, remove_feed_source},
    },
    errors::ApplicationError,
};

env_struct! {
    struct ApplicationEnv {
        log = "true".into(),
    }
}

async fn handle_unknown() -> impl Responder {
    HttpResponse::NotFound()
}

#[actix_web::main]
async fn main() -> Result<(), ApplicationError> {
    let env = ApplicationEnv::load_from_env();
    if matches!(env.log.to_lowercase().as_str(), "true" | "yes" | "ok") {
        formatted_builder()
            // by default enable all logs as Trace >= ALL
            // before deploying consider setting this to OFF
            .filter_level(log::LevelFilter::Trace)
            .parse_default_env()
            .try_init()
            .change_context(ApplicationError::UnexpectedError(
                "Intialized logging twice.",
            ))?;
    }
    const ADDR: (&str, u16) = ("127.0.0.1", 3000);
    println!("Listening on http://{}:{}", ADDR.0, ADDR.1);
    HttpServer::new(|| {
        App::new()
            // send shared sqlx db connection pool
            .app_data(0)
            .wrap(Logger::default())
            .service(get_feed)
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
