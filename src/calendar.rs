use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use google_calendar3::{
    CalendarHub,
    api::{Channel, Event, EventDateTime, Scope},
    hyper_rustls,
    hyper_rustls::HttpsConnector,
    hyper_util,
    hyper_util::client::legacy::connect::HttpConnector,
    yup_oauth2,
    yup_oauth2::ServiceAccountKey,
};

use crate::{
    logic::secrets::{SecretKey, SecretProvider},
    models::Appointment,
};

/// Owns the Google Calendar client and the secret needed to build it.
/// Callers use this instead of threading `&dyn SecretProvider` everywhere.
#[derive(Clone)]
pub(crate) struct CalendarClient {
    secrets: Arc<dyn SecretProvider>,
}

impl CalendarClient {
    pub(crate) fn new(secrets: Arc<dyn SecretProvider>) -> Self {
        Self { secrets }
    }

    async fn calendar_hub(&self) -> Result<CalendarHub<HttpsConnector<HttpConnector>>> {
        let secret: ServiceAccountKey = self
            .secrets
            .get(SecretKey::GoogleCreds)
            .await
            .and_then(|creds| serde_json::from_str(&creds).map_err(anyhow::Error::from))
            .map_err(|e| anyhow!("Error loading credentials: {e}"))?;

        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .unwrap()
            .https_only()
            .enable_http2()
            .build();

        let executor = hyper_util::rt::TokioExecutor::new();
        let auth = yup_oauth2::ServiceAccountAuthenticator::with_client(
            secret,
            yup_oauth2::CustomHyperClientBuilder::from(
                hyper_util::client::legacy::Client::builder(executor.clone())
                    .build(connector.clone()),
            ),
        )
        .build()
        .await?;

        let client = hyper_util::client::legacy::Client::builder(executor).build(connector);

        Ok(CalendarHub::new(client, auth))
    }

    pub(crate) async fn renew_watch(
        &self,
        calendar_id: &str,
        id: &str,
        resource_id: &str,
    ) -> Result<()> {
        // now + 1 year
        let expiration = Utc::now().timestamp_millis() + (1000 * 60 * 60 * 24 * 365);

        let hub = self.calendar_hub().await?;

        // stop the current watch (ignore errors if channel already expired)
        let request = Channel {
            id: Some(id.into()),
            resource_id: Some(resource_id.into()),
            ..Default::default()
        };
        let _ = hub.channels().stop(request).doit().await;

        // add a new watch
        let request = Channel {
            id: Some(id.into()),
            type_: Some("web_hook".into()),
            address: Some("https://backend.sv-eutingen.de/api/calendar/notifications".into()),
            expiration: Some(expiration),
            ..Default::default()
        };
        hub.events()
            .watch(request, calendar_id)
            .add_scope(Scope::Full)
            .doit()
            .await?;

        Ok(())
    }

    pub(crate) async fn appointments(
        &self,
        calendar_id: &str,
        max_results: i32,
    ) -> Result<Vec<Appointment>> {
        let hub = self.calendar_hub().await?;

        let local_datetime = Local::now().with_timezone(&Berlin).naive_local();
        let time_min = Utc.from_local_datetime(&local_datetime).unwrap();

        let (_, events) = hub
            .events()
            .list(calendar_id)
            .add_scope(Scope::EventReadonly)
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
                    let sort_index = index.try_into().with_context(|| {
                        format!("Error converting index {} into sort index", index)
                    })?;
                    into_appointment(event, sort_index)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![],
        };

        Ok(appointments)
    }
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
            Some(date) => date.checked_add_signed(
                Duration::try_days(days_to_add.into())
                    .with_context(|| format!("Cannot create duration of {days_to_add} days."))?,
            ),
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
