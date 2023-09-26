use crate::models::Appointment;
use anyhow::{Context, Result};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use google_calendar3::{
    api::{Channel, Event, EventDateTime},
    CalendarHub,
};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use yup_oauth2::ServiceAccountKey;

const CREDENTIALS: &str = include_str!("../secrets/credentials.json");

async fn calendar_hub() -> Result<CalendarHub<HttpsConnector<HttpConnector>>> {
    let secret: ServiceAccountKey =
        serde_json::from_str(CREDENTIALS).with_context(|| "Error loading credentials")?;

    let auth = yup_oauth2::ServiceAccountAuthenticator::builder(secret)
        .build()
        .await?;

    let hub = CalendarHub::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_only()
                .enable_http2()
                .build(),
        ),
        auth,
    );

    Ok(hub)
}

pub(crate) async fn renew_watch(calendar_id: &str, id: &str, resource_id: &str) -> Result<()> {
    // now + 1 year
    let expiration = Utc::now().timestamp_millis() + (1000 * 60 * 60 * 24 * 365);

    let hub = calendar_hub().await?;

    // stop the current watch
    let request = Channel {
        id: Some(id.into()),
        resource_id: Some(resource_id.into()),
        ..Default::default()
    };
    hub.channels().stop(request).doit().await?;

    // add a new watch
    let request = Channel {
        id: Some(id.into()),
        type_: Some("web_hook".into()),
        address: Some("https://backend2.sv-eutingen.de/api/calendar/notifications".into()),
        expiration: Some(expiration),
        ..Default::default()
    };
    hub.events().watch(request, calendar_id).doit().await?;

    Ok(())
}

pub(crate) async fn appointments(calendar_id: &str, max_results: i32) -> Result<Vec<Appointment>> {
    let hub = calendar_hub().await?;

    let local_datetime = Local::now().with_timezone(&Berlin).naive_local();
    let time_min = Utc.from_local_datetime(&local_datetime).unwrap();

    let (_, events) = hub
        .events()
        .list(calendar_id)
        .max_results(max_results)
        .time_min(time_min)
        .order_by("startTime") //$NON-NLS-1$
        .single_events(true)
        .doit()
        .await?;

    let appointments = match events.items {
        Some(events) => events
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                let sort_index = index
                    .try_into()
                    .with_context(|| format!("Error converting index {} into sort index", index))?;
                into_appointment(event, sort_index)
            })
            .collect::<Result<Vec<_>>>()?,
        None => vec![],
    };

    Ok(appointments)
}

fn into_appointment(event: Event, sort_index: u32) -> Result<Appointment> {
    let start = event.start;
    let end = event.end;
    Ok(Appointment::new(
        event.id,
        sort_index,
        event.summary,
        event.html_link,
        event.description,
        into_date(&start, 0)?,
        into_date(&end, -1)?,
        into_date_time(start)?,
        into_date_time(end)?,
    ))
}

fn into_date(date: &Option<EventDateTime>, days_to_add: i8) -> Result<Option<NaiveDate>> {
    let option = match date {
        Some(value) => match value.date {
            Some(date) => date.checked_add_signed(Duration::days(days_to_add.into())),
            None => None,
        },
        None => None,
    };
    Ok(option)
}

fn into_date_time(date: Option<EventDateTime>) -> Result<Option<NaiveDateTime>> {
    let option = match date {
        Some(value) => value.date_time.map(|s| s.naive_local()),
        None => None,
    };
    Ok(option)
}
