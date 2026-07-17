use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::fmt::{self, Display};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Result, anyhow};
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
use chrono::{NaiveDate, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header,
};
use lambda_http::request::RequestContext;
use serde::de;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing::{debug, error};
use urlencoding::encode;

use crate::calendar::CalendarClient;
use crate::email::RealEmailSender;
use crate::error::ValidationError;
use crate::logic::secrets::{SecretKey, SecretProvider};
use crate::logic::{calendar, contact, events, export, membership, news, tasks};
use crate::models::{
    ContactMessage, Email, EventBooking, EventEmail, EventId, EventType, LifecycleStatus,
    MembershipApplication, NewsSubscription, NewsTopic, PartialEvent,
};

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
        if let Some(message) = err
            .downcast_ref::<ValidationError>()
            .map(|v| v.message.clone())
        {
            return ResponseError {
                err,
                response: Some((StatusCode::BAD_REQUEST, message)),
            };
        }
        ResponseError {
            err,
            response: None,
        }
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        if self.response.is_some() {
            debug!("{:?}", self.err);
        } else {
            error!("{:?}", self);
        }

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

            if let Some(ip_str) = source_ip
                && let Ok(ip) = ip_str.parse::<IpAddr>()
            {
                debug!("Extracted client IP: {}", ip);
                return Ok(ClientIp(Some(ip)));
            }
        }

        Ok(ClientIp(None))
    }
}

pub(crate) async fn router(
    pg_pool: PgPool,
    http_client: reqwest::Client,
    secrets: Arc<dyn SecretProvider>,
) -> Result<Router> {
    let jwks = Arc::new(RwLock::new(JwksCache::new()));

    let email_sender = RealEmailSender::new(secrets.clone());
    let calendar_client = CalendarClient::new(secrets.clone());

    let state = AppState {
        pg_pool,
        jwks,
        http_client: http_client.clone(),
        allowed_emails: vec![
            "fitness@sv-eutingen.de".to_string(),
            "events@sv-eutingen.de".to_string(),
        ],
        allowed_domain: "sv-eutingen.de".to_string(),
        task_api_key: secrets.get(SecretKey::TaskApiKey).await?,
        session_secret: secrets.get(SecretKey::SessionSecret).await?,
        secrets,
        email_sender,
        calendar_client,
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
                        .route("/prebooking/{hash}", get(prebooking))
                        .route("/prebooking/{hash}/iban", post(prebooking_iban)),
                )
                .nest(
                    "/news",
                    Router::new()
                        .route("/subscribe", post(subscribe))
                        .route("/unsubscribe", post(unsubscribe)),
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
                    "/auth",
                    Router::new().route("/session", post(exchange_session)),
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
                                .route("/{id}/sepa_xml", post(export_sepa_xml))
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
                            "/news",
                            Router::new().route("/subscribers", get(subscribers)),
                        )
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
    iat: usize,
}

#[derive(Clone)]
struct AppState {
    pg_pool: PgPool,
    jwks: Arc<RwLock<JwksCache>>,
    http_client: reqwest::Client,
    allowed_emails: Vec<String>,
    allowed_domain: String,
    task_api_key: String,
    session_secret: String,
    secrets: Arc<dyn SecretProvider>,
    email_sender: RealEmailSender,
    calendar_client: CalendarClient,
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

    let claims = match verify_session_jwt(token, &state.session_secret) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Session JWT verification failed: {:?}", e);
            return (StatusCode::UNAUTHORIZED, "Invalid session token").into_response();
        }
    };

    if !authorize(&claims, &state.allowed_emails, &state.allowed_domain) {
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

async fn fetch_jwks(
    client: &reqwest::Client,
    jwks_url: &str,
) -> Result<HashMap<String, Arc<DecodingKey>>> {
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

// auth: session JWT exchange

#[derive(Deserialize)]
struct SessionExchangeRequest {
    google_token: String,
}

#[derive(Serialize)]
struct SessionExchangeResponse {
    token: String,
}

async fn exchange_session(
    State(state): State<AppState>,
    Json(req): Json<SessionExchangeRequest>,
) -> Response {
    let claims = match verify_google_token(&req.google_token, &state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Google token verification failed: {:?}", e);
            return (StatusCode::UNAUTHORIZED, "Invalid Google token").into_response();
        }
    };

    if !authorize(&claims, &state.allowed_emails, &state.allowed_domain) {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    match mint_session_jwt(&claims.email, claims.hd.as_deref(), &state.session_secret) {
        Ok(token) => Json(SessionExchangeResponse { token }).into_response(),
        Err(e) => {
            tracing::error!("Failed to mint session JWT: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to issue session").into_response()
        }
    }
}

async fn verify_google_token(token: &str, state: &AppState) -> Result<Claims> {
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| anyhow!("Missing kid in JWT"))?;

    let decoding_key = {
        let needs_refresh = {
            let jwks_cache = state.jwks.read().await;
            jwks_cache.is_expired() || !jwks_cache.keys.contains_key(&kid)
        };
        if needs_refresh {
            tracing::info!("Refreshing JWKS cache (expired or missing kid: {})", kid);
            let new_keys = fetch_jwks(
                &state.http_client,
                "https://www.googleapis.com/oauth2/v3/certs",
            )
            .await?;
            let mut jwks_cache = state.jwks.write().await;
            jwks_cache.keys = new_keys;
            jwks_cache.last_updated = std::time::Instant::now();
        }
        let jwks_cache = state.jwks.read().await;
        jwks_cache
            .keys
            .get(&kid)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown JWT key"))?
    };

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_aud = false;
    let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

fn mint_session_jwt(email: &str, hd: Option<&str>, secret: &str) -> Result<String> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        email: email.to_string(),
        hd: hd.map(|s| s.to_string()),
        exp: now + 30 * 24 * 60 * 60,
        iat: now,
    };
    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

fn verify_session_jwt(token: &str, secret: &str) -> Result<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    validation.validate_exp = true;
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

fn authorize(claims: &Claims, allowed_emails: &[String], allowed_domain: &str) -> bool {
    allowed_emails.contains(&claims.email) || claims.hd.as_deref() == Some(allowed_domain)
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
pub(crate) struct SubscribersQueryParams {
    topic: String,
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
    validate_captcha(&booking.token, ip, &*state.secrets).await?;
    let response = events::booking(&state.pg_pool, booking, &state.email_sender).await;
    Ok(Json(response))
}

async fn prebooking(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ResponseError> {
    let response = events::prebooking(&state.pg_pool, hash, &state.email_sender).await;
    Ok(Json(response))
}

#[derive(Deserialize)]
struct IbanPayload {
    iban: String,
}

async fn prebooking_iban(
    State(state): State<AppState>,
    Path(hash): Path<String>,
    Json(payload): Json<IbanPayload>,
) -> Result<impl IntoResponse, ResponseError> {
    let response =
        events::prebook_with_iban(&state.pg_pool, &hash, payload.iban, &state.email_sender).await?;
    Ok(Json(response))
}

async fn update(
    State(state): State<AppState>,
    extract::Json(partial_event): extract::Json<PartialEvent>,
) -> Result<impl IntoResponse, ResponseError> {
    let event = events::update(&state.pg_pool, partial_event, &state.email_sender).await?;
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
        match events::update_payment(&state.pg_pool, booking_id, update_payment).await {
            Ok(()) => {}
            Err(e) => {
                if e.downcast_ref::<crate::models::SepaPaymentNotAllowed>()
                    .is_some()
                {
                    return Err(ResponseError {
                        err: e,
                        response: Some((
                            StatusCode::BAD_REQUEST,
                            "Cannot update payment for SEPA bookings".to_string(),
                        )),
                    });
                }
                return Err(e.into());
            }
        }
    }
    Ok(StatusCode::OK)
}

async fn cancel_event_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<i32>,
) -> Result<impl IntoResponse, ResponseError> {
    events::cancel_booking(&state.pg_pool, booking_id, &state.email_sender).await?;
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

async fn export_sepa_xml(
    State(state): State<AppState>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    use crate::models::SepaExportError;

    let (filename, xml) =
        match events::export_sepa_xml(&state.pg_pool, event_id, &*state.secrets).await {
            Ok(result) => result,
            Err(e) => {
                if let Some(sepa_err) = e.downcast_ref::<SepaExportError>() {
                    let status = match sepa_err {
                        SepaExportError::NotASepaEvent => StatusCode::BAD_REQUEST,
                        SepaExportError::NoBookingsAvailable => StatusCode::CONFLICT,
                        SepaExportError::BicLookupFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
                        SepaExportError::ConfigIncomplete => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    let message = e.to_string();
                    return Err(ResponseError {
                        err: e,
                        response: Some((status, message)),
                    });
                }
                return Err(e.into());
            }
        };

    Ok(Response::builder()
        .header("Content-Type", "application/xml")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(xml))
        .unwrap())
}

// news

async fn subscribe(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&subscription.token, ip, &*state.secrets).await?;
    news::subscribe(&state.pg_pool, subscription, &state.email_sender).await?;
    Ok(StatusCode::OK)
}

async fn unsubscribe(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&subscription.token, ip, &*state.secrets).await?;
    news::unsubscribe(&state.pg_pool, subscription).await?;
    Ok(StatusCode::OK)
}

async fn subscribers(
    State(state): State<AppState>,
    query: Query<SubscribersQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    let topics: HashSet<NewsTopic> = query
        .topic
        .split(',')
        .filter_map(|s| NewsTopic::from_str(s).ok())
        .collect();
    let subscriptions = news::get_subscriptions(&state.pg_pool).await?;
    let emails: HashSet<_> = subscriptions
        .into_iter()
        .filter(|(topic, _)| topics.contains(topic))
        .flat_map(|(_, emails)| emails)
        .collect();
    Ok(Json(emails))
}

// calendar

async fn appointments(State(state): State<AppState>) -> Result<impl IntoResponse, ResponseError> {
    let result = calendar::appointments(&state.calendar_client).await?;
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
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(message): Json<ContactMessage>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&message.token, ip, &*state.secrets).await?;
    contact::message(message, &state.email_sender).await?;
    Ok(StatusCode::OK)
}

async fn emails(
    State(state): State<AppState>,
    extract::Json(body): extract::Json<EmailsBody>,
) -> Result<impl IntoResponse, ResponseError> {
    if let Some(emails) = body.emails {
        contact::emails(emails, &state.email_sender).await?;
    } else if let Some(event) = body.event {
        events::send_event_email(&state.pg_pool, event, &state.email_sender).await?;
    }
    Ok(StatusCode::OK)
}

// membership

async fn membership_application(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    extract::Json(application): extract::Json<MembershipApplication>,
) -> Result<impl IntoResponse, ResponseError> {
    validate_captcha(&application.token, ip, &*state.secrets).await?;
    membership::application(&state.pg_pool, application, &state.email_sender).await?;
    Ok(StatusCode::OK)
}

// tasks

async fn check_email_connectivity(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::check_email_connectivity(&state.email_sender).await;
    Ok(StatusCode::OK)
}

async fn renew_calendar_watch(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::renew_calendar_watch(&state.calendar_client).await;
    Ok(StatusCode::OK)
}

async fn send_event_reminders(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_event_reminders(&state.pg_pool, &state.email_sender).await;
    Ok(StatusCode::OK)
}

async fn close_finished_events(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::close_finished_events(&state.pg_pool, &state.email_sender).await;
    Ok(StatusCode::OK)
}

async fn send_payment_reminders(
    State(state): State<AppState>,
    Path(event_type): Path<EventType>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_payment_reminders(&state.pg_pool, event_type, &state.email_sender).await?;
    Ok(StatusCode::OK)
}

async fn send_participation_confirmation(
    State(state): State<AppState>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_participation_confirmation(&state.pg_pool, event_id, &state.email_sender).await?;
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
    secrets: &dyn SecretProvider,
) -> Result<(), ResponseError> {
    let token = token.as_ref().ok_or_else(|| ResponseError {
        err: anyhow::anyhow!("No captcha token provided"),
        response: Some((StatusCode::BAD_REQUEST, "Captcha token is required.".into())),
    })?;

    let secret = &secrets.get(SecretKey::CaptchaSecret).await?;

    let captcha = hcaptcha::Captcha::new(token.as_str()).map_err(|e| ResponseError {
        err: anyhow::anyhow!("Failed to create captcha: {:?}", e),
        response: Some((StatusCode::BAD_REQUEST, "Invalid captcha token.".into())),
    })?;

    let mut request = hcaptcha::Request::new(secret, captcha).map_err(|e| ResponseError {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_secret() -> String {
        "test-secret-key-that-is-at-least-32-bytes!".to_string()
    }

    #[test]
    fn test_session_jwt_round_trip() {
        let secret = test_secret();
        let token =
            mint_session_jwt("admin@sv-eutingen.de", Some("sv-eutingen.de"), &secret).unwrap();
        let claims = verify_session_jwt(&token, &secret).unwrap();
        assert_eq!(claims.email, "admin@sv-eutingen.de");
        assert_eq!(claims.hd.as_deref(), Some("sv-eutingen.de"));
        assert!(claims.exp > claims.iat);
        assert_eq!(claims.exp - claims.iat, 30 * 24 * 60 * 60);
    }

    #[test]
    fn test_verify_rejects_expired_session_jwt() {
        let secret = test_secret();
        let claims = Claims {
            email: "admin@sv-eutingen.de".to_string(),
            hd: Some("sv-eutingen.de".to_string()),
            exp: 1,
            iat: 0,
        };
        let token = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        assert!(verify_session_jwt(&token, &secret).is_err());
    }

    #[test]
    fn test_verify_rejects_bad_signature() {
        let secret = test_secret();
        let wrong_secret = "a-completely-different-secret-key!".to_string();
        let token =
            mint_session_jwt("admin@sv-eutingen.de", Some("sv-eutingen.de"), &secret).unwrap();
        assert!(verify_session_jwt(&token, &wrong_secret).is_err());
    }

    #[test]
    fn test_authorize_allows_allowlisted_email() {
        let claims = Claims {
            email: "fitness@sv-eutingen.de".to_string(),
            hd: None,
            exp: 0,
            iat: 0,
        };
        let allowed_emails = vec!["fitness@sv-eutingen.de".to_string()];
        assert!(authorize(&claims, &allowed_emails, "sv-eutingen.de"));
    }

    #[test]
    fn test_authorize_allows_matching_hosted_domain() {
        let claims = Claims {
            email: "someone@sv-eutingen.de".to_string(),
            hd: Some("sv-eutingen.de".to_string()),
            exp: 0,
            iat: 0,
        };
        let allowed_emails = vec!["fitness@sv-eutingen.de".to_string()];
        assert!(authorize(&claims, &allowed_emails, "sv-eutingen.de"));
    }

    #[test]
    fn test_authorize_rejects_unknown_email_and_mismatched_domain() {
        let claims = Claims {
            email: "random@example.com".to_string(),
            hd: Some("example.com".to_string()),
            exp: 0,
            iat: 0,
        };
        let allowed_emails = vec!["fitness@sv-eutingen.de".to_string()];
        assert!(!authorize(&claims, &allowed_emails, "sv-eutingen.de"));
    }
}
