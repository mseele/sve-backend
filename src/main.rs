#[macro_use]
extern crate base64_serde;

mod api;
mod calendar;
mod email;
mod logic;
mod models;
mod sheets;
mod store;

use actix_web::{dev::Service, web, App, HttpServer};
use log::error;

pub const CREDENTIALS: &str = include_str!("../data/credentials.json");

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    HttpServer::new(|| {
        App::new()
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
            .service(web::scope("/api").configure(api::config))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
