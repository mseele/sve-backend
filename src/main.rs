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

use axum::response::IntoResponse;
use lambda_http::Error;
use std::{future::Future, pin::Pin};
use tower::{Layer, ServiceBuilder};
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_service::Service;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time(/* cloudwatch does that */)
        .json()
        .flatten_event(true)
        .init();

    let pool = db::init_pool().await?;

    let app = api::router(pool).layer(
        ServiceBuilder::new().layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        ),
    );

    if cfg!(debug_assertions) {
        Ok(axum::Server::bind(&"0.0.0.0:8080".parse()?)
            .serve(app.into_make_service())
            .await?)
    } else {
        let app = ServiceBuilder::new().layer(LambdaLayer).service(app);

        Ok(lambda_http::run(app).await?)
    }
}

#[derive(Clone, Copy)]
pub struct LambdaLayer;

impl<S> Layer<S> for LambdaLayer {
    type Service = LambdaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LambdaService { inner }
    }
}

pub struct LambdaService<S> {
    inner: S,
}

impl<S> Service<lambda_http::Request> for LambdaService<S>
where
    S: Service<axum::http::Request<axum::body::Body>>,
    S::Response: axum::response::IntoResponse + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    type Response = lambda_http::Response<lambda_http::Body>;
    type Error = lambda_http::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: lambda_http::Request) -> Self::Future {
        let (parts, body) = req.into_parts();
        let body = match body {
            lambda_http::Body::Empty => axum::body::Body::default(),
            lambda_http::Body::Text(t) => t.into(),
            lambda_http::Body::Binary(v) => v.into(),
        };

        let request = axum::http::Request::from_parts(parts, body);

        let fut = self.inner.call(request);
        let fut = async move {
            let resp = fut.await?;
            let (parts, body) = resp.into_response().into_parts();
            let bytes = hyper::body::to_bytes(body).await?;
            let bytes: &[u8] = &bytes;
            let resp: hyper::Response<lambda_http::Body> = match std::str::from_utf8(bytes) {
                Ok(s) => hyper::Response::from_parts(parts, s.into()),
                Err(_) => hyper::Response::from_parts(parts, bytes.into()),
            };
            Ok(resp)
        };

        Box::pin(fut)
    }
}
