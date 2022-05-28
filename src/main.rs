#[macro_use]
extern crate base64_serde;

mod api;
mod calendar;
mod db;
mod email;
mod logic;
mod models;
mod sheets;
mod store;

use actix_cors::Cors;
use actix_web::{dev::Service, web, App, HttpServer};
use log::error;

pub const CREDENTIALS: &str = include_str!("../secrets/credentials.json");

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let pool = db::init_pool().await?;

    HttpServer::new(move || {
        App::new()
            // Access-Control-Allow-Origin
            .wrap(
                Cors::default()
                    .send_wildcard()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600),
            )
            // Log errors
            .wrap_fn(|req, srv| {
                let fut = srv.call(req);
                async {
                    let res = fut.await?;
                    if let Some(error) = res.response().error() {
                        error!("{:?}", error);
                    }
                    Ok(res)
                }
            })
            .app_data(web::Data::new(pool.clone()))
            .service(web::scope("/api").configure(api::config))
    })
    // Configure workers manually because inside the cloud
    // the num of cpu cores will return 1
    .workers(4)
    .bind("0.0.0.0:8080")?
    .run()
    .await?;

    Ok(())
}
