#[macro_use]
extern crate base64_serde;

mod api;
mod calendar;
mod db;
mod email;
mod logic;
mod models;
mod hashids {
    use harsh::{Error, Harsh};

    fn harsh() -> Harsh {
        Harsh::builder()
            .salt("#mehralseinverein")
            .length(10)
            .alphabet("abcdefghijklmnopqrstuvwxyz")
            .build()
            .unwrap()
    }

    pub(crate) fn encode(values: &[u64]) -> String {
        harsh().encode(values)
    }

    pub(crate) fn decode<T: AsRef<str>>(input: T) -> Result<Vec<u64>, Error> {
        harsh().decode(input)
    }
}

use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub(crate) const CREDENTIALS: &str = include_str!("../secrets/credentials.json");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let pool = db::init_pool().await?;

    let app = api::router(pool).layer(
        ServiceBuilder::new()
            .layer(
                TraceLayer::new_for_http()
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .layer(CorsLayer::permissive().max_age(Duration::from_secs(3600))),
    );

    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
