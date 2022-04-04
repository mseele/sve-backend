use crate::models::Appointment;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Europe::Berlin;
use google_calendar3::{
    api::{Channel, Event, EventDateTime},
    CalendarHub,
};
use std::time::{SystemTime, UNIX_EPOCH};
use yup_oauth2::ServiceAccountKey;

async fn calendar_hub() -> Result<CalendarHub> {
    let secret: ServiceAccountKey =
        serde_json::from_str(crate::CREDENTIALS).with_context(|| "Error loading credentials")?;

    let auth = yup_oauth2::ServiceAccountAuthenticator::builder(secret)
        .build()
        .await?;

    let hub = CalendarHub::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

    Ok(hub)
}

pub async fn renew_watch(calendar_id: &str, id: &str, resource_id: &str) -> Result<()> {
    // now + 1 year
    let expiration =
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() + (1000 * 60 * 60 * 24 * 365);

    let hub = calendar_hub().await?;

    // stop the current watch
    let mut request = Channel::default();
    request.id = Some(id.into());
    request.resource_id = Some(resource_id.into());
    hub.channels().stop(request).doit().await?;

    // add a new watch
    let mut request = Channel::default();
    request.id = Some(id.into());
    request.type_ = Some("web_hook".into());
    request.address = Some("https://sve-backend.appspot.com/api/calendar/notifications".into());
    request.expiration = Some(format!("{}", expiration));
    hub.events().watch(request, calendar_id).doit().await?;

    Ok(())
}

pub async fn appointments(calendar_id: &str, max_results: i32) -> Result<Vec<Appointment>> {
    let hub = calendar_hub().await?;

    let time_min = Utc::now().with_timezone(&Berlin).to_rfc3339();

    let (_, events) = hub
        .events()
        .list(calendar_id)
        .max_results(max_results)
        .time_min(&time_min)
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
                Ok(into_appointment(event, sort_index)?)
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
        Some(value) => match &value.date {
            Some(s) => {
                let result = NaiveDate::parse_from_str(&s, "%Y-%m-%d")?;
                result.checked_add_signed(Duration::days(days_to_add.into()))
            }
            None => None,
        },
        None => None,
    };
    Ok(option)
}

fn into_date_time(date: Option<EventDateTime>) -> Result<Option<NaiveDateTime>> {
    let option = match date {
        Some(value) => match value.date_time {
            Some(s) => Some(DateTime::parse_from_rfc3339(&s)?.naive_local()),
            None => None,
        },
        None => None,
    };
    Ok(option)
}
