use crate::logic::{calendar, contact, events, export, membership, news, secrets, tasks};
use crate::models::{
    ContactMessage, Email, EventBooking, EventEmail, EventId, EventType, LifecycleStatus,
    MembershipApplication, NewsSubscription, PartialEvent,
};
use anyhow::Result;
use axum::body::Body;
use axum::extract::{self, FromRequestParts, Path, Query, State};
use axum::http::{
    Request, StatusCode,
    header::{self, HeaderMap},
    request::Parts,
};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use lambda_http::request::RequestContext;
use reqwest::Client;
use serde::de;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::{self, Display};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use tracing::{debug, error};
use urlencoding::encode;

pub(crate) struct ResponseError {
    err: anyhow::Error,
    response: Option<(StatusCode, String)>,
}

impl Error for ResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.err.source()
    }

    //TODO: what to do with backtrace
    // fn backtrace(&self) -> Option<&std::backtrace::Backtrace> {
    //    self.err.backtrace()
    // }

    fn description(&self) -> &str {
        #[allow(deprecated)]
        self.err.description()
    }

    fn cause(&self) -> Option<&dyn Error> {
        #[allow(deprecated)]
        self.err.cause()
    }
}

impl Debug for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl From<anyhow::Error> for ResponseError {
    fn from(err: anyhow::Error) -> ResponseError {
        ResponseError {
            err,
            response: None,
        }
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        error!("{:?}", self);

        self.response
            .unwrap_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred. Please try again later.".to_string(),
                )
            })
            .into_response()
    }
}

pub(crate) struct ClientIp(pub(crate) Option<IpAddr>);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        if let Some(ctx) = parts.extensions.get::<RequestContext>() {
            let source_ip = match ctx {
                RequestContext::ApiGatewayV2(ctx) => ctx.http.source_ip.as_deref(),
                _ => None,
            };

            if let Some(ip_str) = source_ip {
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    debug!("Extracted client IP: {}", ip);
                    return Ok(ClientIp(Some(ip)));
                }
            }
        }

        Ok(ClientIp(None))
    }
}

pub(crate) async fn router(pg_pool: PgPool) -> Result<Router> {
    let mut jwks_cache = JwksCache::new();
    jwks_cache.keys = fetch_jwks("https://www.googleapis.com/oauth2/v3/certs").await?;
    let jwks = Arc::new(RwLock::new(jwks_cache));

    let state = AppState {
        pg_pool,
        jwks,
        allowed_emails: vec![
            "fitness@sv-eutingen.de".to_string(),
            "events@sv-eutingen.de".to_string(),
        ],
        allowed_domain: "sv-eutingen.de".to_string(),
        task_api_key: secrets::get("TASK_API_KEY").await?,
    };

    Ok(Router::new()
        .nest(
            "/api",
            Router::new()
                .nest(
                    "/events",
                    Router::new()
                        .route("/", get(events))
                        .route("/custom_fields", get(custom_fields))
                        .route("/counter", get(counter))
                        .route("/booking", post(booking))
                        .route("/prebooking/{hash}", get(prebooking)),
                )
                .nest(
                    "/news",
                    Router::new()
                        .route("/subscribe", post(subscribe))
                        .route("/unsubscribe", post(unsubscribe))
                        .route("/subscribers", get(subscribers)),
                )
                .nest("/contact", Router::new().route("/message", post(message)))
                .nest(
                    "/calendar",
                    Router::new()
                        .route("/appointments", get(appointments))
                        .route("/notifications", post(notifications)),
                )
                .nest(
                    "/membership",
                    Router::new().route("/application", post(membership_application)),
                )
                .nest(
                    "/tasks",
                    Router::new()
                        .route("/check_email_connectivity", get(check_email_connectivity))
                        .route("/renew_calendar_watch", get(renew_calendar_watch))
                        .route("/send_event_reminders", get(send_event_reminders))
                        .route("/close_finished_events", get(close_finished_events))
                        .layer(axum::middleware::from_fn_with_state(
                            state.clone(),
                            api_key_middleware_fn,
                        )),
                )
                .nest(
                    "/admin",
                    Router::new()
                        .nest(
                            "/events",
                            Router::new()
                                .route("/", get(admin_events))
                                .route("/update", post(update))
                                .route("/{id}", delete(delete_event))
                                .nest(
                                    "/booking",
                                    Router::new()
                                        .route(
                                            "/{id}",
                                            patch(update_event_booking)
                                                .delete(cancel_event_booking),
                                        )
                                        .route("/export/{event_id}", get(export_event_bookings))
                                        .route(
                                            "/participants_list/{event_id}",
                                            get(export_event_participants_list),
                                        ),
                                )
                                .nest(
                                    "/payments",
                                    Router::new()
                                        .route("/verify", post(verify_payments))
                                        .route("/unpaid/{event_type}", get(unpaid_bookings)),
                                ),
                        )
                        .nest("/contact", Router::new().route("/emails", post(emails)))
                        .nest(
                            "/tasks",
                            Router::new()
                                .route(
                                    "/send_payment_reminders/{event_type}",
                                    get(send_payment_reminders),
                                )
                                .route(
                                    "/send_participation_confirmation/{event_id}",
                                    get(send_participation_confirmation),
                                ),
                        )
                        .layer(axum::middleware::from_fn_with_state(
                            state.clone(),
                            auth_middleware_fn,
                        )),
                ),
        )
        .with_state(state))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    email: String,
    hd: Option<String>, // Hosted domain (Google Workspace domain)
    exp: usize,
}

#[derive(Clone)]
struct AppState {
    pg_pool: PgPool,
    jwks: Arc<RwLock<JwksCache>>,
    allowed_emails: Vec<String>,
    allowed_domain: String,
    task_api_key: String,
}

#[derive(Clone)]
struct JwksCache {
    keys: HashMap<String, Arc<DecodingKey>>,
    last_updated: std::time::Instant,
    ttl: std::time::Duration,
}

impl JwksCache {
    fn new() -> Self {
        Self {
            keys: HashMap::new(),
            last_updated: std::time::Instant::now(),
            ttl: std::time::Duration::from_secs(24 * 3600), // 24 hours
        }
    }

    fn is_expired(&self) -> bool {
        self.last_updated.elapsed() > self.ttl
    }
}

async fn auth_middleware_fn(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let auth_header = match req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
    {
        Some(h) => h,
        None => return (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response(),
    };

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Invalid Authorization header").into_response(),
    };

    let header = match decode_header(token) {
        Ok(h) => h,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid JWT header").into_response(),
    };

    let kid = match header.kid {
        Some(k) => k,
        None => return (StatusCode::UNAUTHORIZED, "Missing kid in JWT").into_response(),
    };

    let decoding_key = {
        let needs_refresh = {
            let jwks_cache = state.jwks.read().unwrap();
            jwks_cache.is_expired() || !jwks_cache.keys.contains_key(&kid)
        };
        if needs_refresh {
            tracing::info!("Refreshing JWKS cache (expired or missing kid: {})", kid);
            match fetch_jwks("https://www.googleapis.com/oauth2/v3/certs").await {
                Ok(new_keys) => {
                    let mut jwks_cache = state.jwks.write().unwrap();
                    jwks_cache.keys = new_keys;
                    jwks_cache.last_updated = std::time::Instant::now();
                }
                Err(e) => {
                    tracing::error!("Failed to refresh JWKS: {:?}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "JWKS refresh failed")
                        .into_response();
                }
            }
        }
        let jwks_cache = state.jwks.read().unwrap();
        match jwks_cache.keys.get(&kid).cloned() {
            Some(k) => k,
            None => return (StatusCode::UNAUTHORIZED, "Unknown JWT key").into_response(),
        }
    };

    let mut validation = Validation::new(Algorithm::RS256);
    // Since we trust Google's signature verification,
    // the audience check is redundant here.
    validation.validate_aud = false;
    let token_data = match decode::<Claims>(token, &*decoding_key, &validation) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("JWT decode error: {:?}", e);
            return (StatusCode::UNAUTHORIZED, "Invalid JWT").into_response();
        }
    };

    let claims = token_data.claims;

    // Restrict access to specific emails or domain
    if !state.allowed_emails.contains(&claims.email)
        && claims.hd.as_deref() != Some(&state.allowed_domain)
    {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    next.run(req).await
}

async fn api_key_middleware_fn(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let api_key = req.headers().get("x-api-key").and_then(|h| h.to_str().ok());

    match api_key {
        Some(key) if key == state.task_api_key => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    }
}

async fn fetch_jwks(jwks_url: &str) -> Result<HashMap<String, Arc<DecodingKey>>> {
    let client = Client::new();
    let res = client
        .get(jwks_url)
        .send()
        .await?
        .json::<HashMap<String, Vec<Jwk>>>()
        .await?;

    let mut keys = HashMap::new();
    if let Some(jwks_keys) = res.get("keys") {
        for key in jwks_keys {
            if let (Some(kid), Some(n), Some(e)) = (&key.kid, &key.n, &key.e) {
                let decoding_key = DecodingKey::from_rsa_components(n, e).unwrap();
                keys.insert(kid.clone(), Arc::new(decoding_key));
            }
        }
    }

    Ok(keys)
}

#[derive(Debug, Serialize, Deserialize)]
struct Jwk {
    kid: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

// events

pub(crate) fn deserialize_lifecycle_status_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<LifecycleStatus>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringVecVisitor;

    impl de::Visitor<'_> for StringVecVisitor {
        type Value = Option<Vec<LifecycleStatus>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter
                .write_str("a string containing a comma separated list of lifecycle status strings")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let mut list = Vec::new();
            for status in v.split(',') {
                list.push(LifecycleStatus::from_str(status).map_err(E::custom)?);
            }
            Ok(match list.len() {
                0 => None,
                _ => Some(list),
            })
        }
    }

    deserializer.deserialize_any(StringVecVisitor)
}

#[derive(Debug, Deserialize)]
struct EventsQueryParams {
    beta: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_lifecycle_status_list")]
    status: Option<Vec<LifecycleStatus>>,
    subscribers: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EventCountersQueryParams {
    beta: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VerifyPaymentInput {
    csv: String,
    start_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEventBookingQueryParams {
    update_payment: Option<bool>,
}

async fn events(
    State(state): State<AppState>,
    mut query: Query<EventsQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    let events = events::get_events(
        &state.pg_pool,
        query.beta.take(),
        query.status.take(),
        Some(false), // Public endpoint never returns subscribers
    )
    .await?;
    Ok(Json(events))
}

async fn admin_events(
    State(state): State<AppState>,
    query: Query<EventsQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    let events = events::get_events(
        &state.pg_pool,
        query.beta,
        query.status.clone(),
        query.subscribers,
    )
    .await?;
    Ok(Json(events))
}

async fn custom_fields(State(state): State<AppState>) -> Result<impl IntoResponse, ResponseError> {
    let custom_fields = events::get_all_custom_fields(&state.pg_pool).await?;
    Ok(Json(custom_fields))
}

async fn counter(
    State(state): State<AppState>,
    query: Query<EventCountersQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    let event_counters = events::get_event_counters(&state.pg_pool, query.beta).await?;
    Ok(Json(event_counters))
}

async fn booking(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(booking): extract::Json<EventBooking>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&booking.token, ip).await?;
    let response = events::booking(&state.pg_pool, booking).await;
    Ok(Json(response))
}

async fn prebooking(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ResponseError> {
    let response = events::prebooking(&state.pg_pool, hash).await;
    Ok(Json(response))
}

async fn update(
    State(state): State<AppState>,
    extract::Json(partial_event): extract::Json<PartialEvent>,
) -> Result<impl IntoResponse, ResponseError> {
    let event = events::update(&state.pg_pool, partial_event).await?;
    Ok(Json(event))
}

async fn delete_event(
    State(state): State<AppState>,
    Path(path): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    events::delete(&state.pg_pool, path).await?;
    Ok(StatusCode::OK)
}

async fn verify_payments(
    State(state): State<AppState>,
    extract::Json(input): extract::Json<VerifyPaymentInput>,
) -> Result<impl IntoResponse, ResponseError> {
    Ok(Json(
        events::verify_payments(&state.pg_pool, input.csv, input.start_date).await?,
    ))
}

async fn unpaid_bookings(
    State(state): State<AppState>,
    Path(event_type): Path<EventType>,
) -> Result<impl IntoResponse, ResponseError> {
    Ok(Json(
        events::get_unpaid_bookings(&state.pg_pool, event_type).await?,
    ))
}

async fn update_event_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<i32>,
    query: Query<UpdateEventBookingQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    if let Some(update_payment) = query.update_payment {
        events::update_payment(&state.pg_pool, booking_id, update_payment).await?;
    }
    Ok(StatusCode::OK)
}

async fn cancel_event_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<i32>,
) -> Result<impl IntoResponse, ResponseError> {
    events::cancel_booking(&state.pg_pool, booking_id).await?;
    Ok(StatusCode::OK)
}

async fn export_event_bookings(
    State(state): State<AppState>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    let (filename, bytes) = export::event_bookings(&state.pg_pool, event_id).await?;
    Ok(into_file_response(filename, bytes))
}

async fn export_event_participants_list(
    State(state): State<AppState>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    let (filename, bytes) = export::event_participants_list(&state.pg_pool, event_id).await?;
    Ok(into_file_response(filename, bytes))
}

// news

async fn subscribe(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&subscription.token, ip).await?;
    news::subscribe(&state.pg_pool, subscription).await?;
    Ok(StatusCode::OK)
}

async fn unsubscribe(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&subscription.token, ip).await?;
    news::unsubscribe(&state.pg_pool, subscription).await?;
    Ok(StatusCode::OK)
}

async fn subscribers(State(state): State<AppState>) -> Result<impl IntoResponse, ResponseError> {
    let subscriptions = news::get_subscriptions(&state.pg_pool).await?;
    let result = subscriptions
        .into_iter()
        .map(|(topic, emails)| {
            let title: &str = &format!(
                "---------- {}: {} ----------",
                topic.display_name(),
                emails.len()
            );
            let mut chunks = vec![];
            for chunk in emails.into_iter().collect::<Vec<_>>().chunks(300) {
                chunks.push(chunk.join(";"));
            }
            [title, &chunks.join("<br/><br/>"), title].join("<br/>")
        })
        .collect::<Vec<_>>()
        .join("<br/><br/><br/>");

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html")],
        result,
    ))
}

// calendar

async fn appointments() -> Result<impl IntoResponse, ResponseError> {
    let result = calendar::appointments().await?;
    Ok(Json(result))
}

async fn notifications(headers: HeaderMap) -> Result<impl IntoResponse, ResponseError> {
    let header_key = "X-Goog-Channel-Id";
    let channel_id = headers.get(header_key);
    if let Some(channel_id) = channel_id {
        match channel_id.to_str() {
            Ok(channel_id) => calendar::notifications(channel_id).await?,
            Err(e) => error!(
                "Could not parse header '{}' into a str: {:?}",
                header_key, e
            ),
        }
    } else {
        error!("Header '{}' has not been found in the request", header_key);
    }
    Ok(StatusCode::OK)
}

// contact

#[derive(Deserialize, Debug)]
struct EmailsBody {
    emails: Option<Vec<Email>>,
    event: Option<EventEmail>,
}

async fn message(
    ClientIp(ip): ClientIp,
    Json(message): Json<ContactMessage>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&message.token, ip).await?;
    contact::message(message).await?;
    Ok(StatusCode::OK)
}

async fn emails(
    State(state): State<AppState>,
    extract::Json(body): extract::Json<EmailsBody>,
) -> Result<impl IntoResponse, ResponseError> {
    if let Some(emails) = body.emails {
        contact::emails(emails).await?;
    } else if let Some(event) = body.event {
        events::send_event_email(&state.pg_pool, event).await?;
    }
    Ok(StatusCode::OK)
}

// membership

async fn membership_application(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(application): extract::Json<MembershipApplication>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&application.token, ip).await?;
    membership::application(&state.pg_pool, application).await?;
    Ok(StatusCode::OK)
}

// tasks

async fn check_email_connectivity() -> Result<impl IntoResponse, ResponseError> {
    tasks::check_email_connectivity().await;
    Ok(StatusCode::OK)
}

async fn renew_calendar_watch() -> Result<impl IntoResponse, ResponseError> {
    tasks::renew_calendar_watch().await;
    Ok(StatusCode::OK)
}

async fn send_event_reminders(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_event_reminders(&state.pg_pool).await;
    Ok(StatusCode::OK)
}

async fn close_finished_events(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::close_finished_events(&state.pg_pool).await;
    Ok(StatusCode::OK)
}

async fn send_payment_reminders(
    State(state): State<AppState>,
    Path(event_type): Path<EventType>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_payment_reminders(&state.pg_pool, event_type).await?;
    Ok(StatusCode::OK)
}

async fn send_participation_confirmation(
    State(state): State<AppState>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_participation_confirmation(&state.pg_pool, event_id).await?;
    Ok(StatusCode::OK)
}

fn into_file_response(filename: String, bytes: Vec<u8>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", encode(&filename)),
        )],
        bytes,
    )
}

/// Validates the provided captcha token using the hCaptcha service.
/// Returns Ok(()) if the captcha is valid, or a ResponseError if validation fails.
async fn validate_captcha(
    token: &Option<String>,
    client_ip: Option<IpAddr>,
) -> Result<(), ResponseError> {
    let token = token.as_ref().ok_or_else(|| ResponseError {
        err: anyhow::anyhow!("No captcha token provided"),
        response: Some((StatusCode::BAD_REQUEST, "Captcha token is required.".into())),
    })?;

    let secret = &secrets::get("CAPTCHA_SECRET").await?;

    let captcha = hcaptcha::Captcha::new(token.as_str()).map_err(|e| ResponseError {
        err: anyhow::anyhow!("Failed to create captcha: {:?}", e),
        response: Some((StatusCode::BAD_REQUEST, "Invalid captcha token.".into())),
    })?;

    let mut request = hcaptcha::Request::new(&secret, captcha).map_err(|e| ResponseError {
        err: anyhow::anyhow!("Failed to build captcha request: {:?}", e),
        response: Some((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Captcha validation failed.".into(),
        )),
    })?;

    if let Some(ip) = client_ip {
        request = request
            .set_remoteip(&ip.to_string())
            .map_err(|e| ResponseError {
                err: anyhow::anyhow!("Failed to build captcha request: {:?}", e),
                response: Some((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Captcha validation failed.".into(),
                )),
            })?;
    }

    let response = hcaptcha::Client::new()
        .verify(request)
        .await
        .map_err(|e| ResponseError {
            err: anyhow::anyhow!("Captcha verification failed: {:?}", e),
            response: Some((
                StatusCode::BAD_REQUEST,
                "Captcha verification failed.".into(),
            )),
        })?;

    if response.success() {
        Ok(())
    } else {
        Err(ResponseError {
            err: anyhow::anyhow!("Captcha invalid: {:?}", response.error_codes()),
            response: Some((StatusCode::BAD_REQUEST, "Invalid captcha token.".into())),
        })
    }
}
