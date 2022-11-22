use std::collections::HashMap;

use crate::{
    db,
    models::{Event, EventId, EventSubscription, ToEuro},
};
use anyhow::{anyhow, Result};
use simple_excel_writer::{row, CellValue, Column, Row, ToCellValue, Workbook};
use sqlx::PgPool;

/// run an excel export for the event bookings of the given event id
pub(crate) async fn event_bookings(pool: &PgPool, event_id: EventId) -> Result<(String, Vec<u8>)> {
    // fetch the event with all subscribers
    let mut event = db::get_event(pool, &event_id, true)
        .await?
        .ok_or_else(|| anyhow!("Error fetching event with id '{}'", event_id.get_ref()))?;
    let subscribers = event.subscribers.take().ok_or_else(|| {
        anyhow!(
            "Subscribers of event with id '{}' are missing",
            event_id.get_ref()
        )
    })?;
    // separate the subscribers into bookings and waiting list
    let mut bookings = Vec::new();
    let mut waiting_list = Vec::new();
    for subscriber in subscribers.into_iter() {
        if subscriber.enrolled {
            bookings.push(subscriber);
        } else {
            waiting_list.push(subscriber);
        }
    }

    actix_web::rt::spawn(async { export(event, bookings, waiting_list) }).await?
}

fn export(
    event: Event,
    bookings: Vec<EventSubscription>,
    waiting_list: Vec<EventSubscription>,
) -> Result<(String, Vec<u8>)> {
    let mut workbook = Workbook::create_in_memory();

    // create one sheet for bookings and one for waiting list
    create_sheet(&mut workbook, "Buchungen", &event, bookings)?;
    create_sheet(&mut workbook, "Warteliste", &event, waiting_list)?;

    // close the workbook and extract the bytes
    let bytes = workbook
        .close()?
        .ok_or_else(|| anyhow!("Workbook did not return some bytes"))?;

    // create a filename and return it with the bytes
    let filename = format!("{}.xlsx", event.name.replace(' ', "_").to_lowercase());

    Ok((filename, bytes))
}

fn create_sheet(
    workbook: &mut Workbook,
    name: &str,
    event: &Event,
    subscribers: Vec<EventSubscription>,
) -> Result<()> {
    let mut sheet = workbook.create_sheet(&format!("{name} ({})", subscribers.len()));

    sheet.add_column(Column { width: 5.0 });
    sheet.add_column(Column { width: 15.0 });
    sheet.add_column(Column { width: 16.0 });
    sheet.add_column(Column { width: 16.0 });
    sheet.add_column(Column { width: 18.0 });
    sheet.add_column(Column { width: 20.0 });
    sheet.add_column(Column { width: 25.0 });
    sheet.add_column(Column { width: 20.0 });
    sheet.add_column(Column { width: 8.0 });
    sheet.add_column(Column { width: 10.0 });
    sheet.add_column(Column { width: 10.0 });
    sheet.add_column(Column { width: 6.5 });
    sheet.add_column(Column { width: 100.0 });

    workbook.write_sheet(&mut sheet, |sheet_writer| {
        sheet_writer.append_row(row![
            "Id",
            "Buchungsdatum",
            "Vorname",
            "Nachname",
            "Straße",
            "PLZ",
            "Email",
            "Telefon",
            "Mitglied",
            "Betrag",
            "Buchungsnr",
            "Bezahlt",
            "Kommentar"
        ])?;

        for value in subscribers.into_iter() {
            sheet_writer.append_row(row![
                value.id.to_string(),
                value.created.naive_utc(),
                value.first_name,
                value.last_name,
                value.street,
                value.city,
                value.email,
                opt(value.phone),
                bool(value.member),
                event.cost(value.member).to_euro(),
                value.payment_id,
                bool(value.payed),
                opt(value.comment)
            ])?;
        }

        Ok(())
    })?;

    Ok(())
}

fn opt<T>(value: Option<T>) -> CellValue
where
    T: ToCellValue,
{
    match value {
        Some(v) => v.to_cell_value(),
        None => CellValue::Blank(1),
    }
}

fn bool(value: bool) -> CellValue {
    match value {
        true => CellValue::String("Y".into()),
        false => CellValue::String("N".into()),
    }
}
