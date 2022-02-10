mod api;
mod calendar;
mod models;
mod sheets;
mod store;

use actix_web::{middleware::Logger, web, App, HttpServer};

pub const CREDENTIALS: &str = include_str!("../data/credentials.json");

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(web::scope("/api").configure(api::events::config))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
