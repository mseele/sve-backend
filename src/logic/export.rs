use std::{collections::HashMap, fs};

use crate::{
    db,
    models::{Event, EventId, EventSubscription, ToEuro},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Timelike, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use xlsxwriter::{Format, Workbook, Worksheet};

const FONT_SIZE: f64 = 12.0;

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
    // create a new workbook with a random name
    let uuid = Uuid::new_v4().to_string();
    let workbook = Workbook::new(&uuid);

    let fmt = workbook
        .add_format()
        .set_text_wrap()
        .set_font_size(FONT_SIZE);

    let bold_fmt = workbook.add_format().set_bold().set_font_size(FONT_SIZE);

    let date_fmt = workbook
        .add_format()
        .set_num_format("dd.mm.yyyy HH:mm:ss")
        .set_font_size(FONT_SIZE);

    // create one sheet for bookings and one for waiting list
    create_sheet(
        &workbook,
        "Buchungen",
        &event,
        bookings,
        &fmt,
        &bold_fmt,
        &date_fmt,
    )?;
    create_sheet(
        &workbook,
        "Warteliste",
        &event,
        waiting_list,
        &fmt,
        &bold_fmt,
        &date_fmt,
    )?;

    // close the workbook and extract the bytes
    workbook.close()?;
    let bytes = fs::read(&uuid)?;
    fs::remove_file(&uuid)?;

    // create a filename and return it with the bytes
    let filename = format!("{}.xlsx", event.name.replace(' ', "_").to_lowercase());

    Ok((filename, bytes))
}

fn create_sheet(
    workbook: &Workbook,
    name: &str,
    event: &Event,
    subscribers: Vec<EventSubscription>,
    fmt: &Format,
    bold_fmt: &Format,
    date_fmt: &Format,
) -> Result<()> {
    let mut width_map: HashMap<u16, usize> = HashMap::new();

    let mut sheet = workbook.add_worksheet(Some(&format!("{name} ({})", subscribers.len())))?;
    create_headers(&mut sheet, bold_fmt, &mut width_map);

    for (i, value) in subscribers.into_iter().enumerate() {
        add_row(
            i as u32,
            event,
            &value,
            &mut sheet,
            date_fmt,
            &mut width_map,
        );
    }

    width_map.iter().for_each(|(k, v)| {
        let _ = sheet.set_column(*k as u16, *k as u16, *v as f64 * 1.2, Some(fmt));
    });

    Ok(())
}

fn add_row(
    row: u32,
    event: &Event,
    value: &EventSubscription,
    sheet: &mut Worksheet,
    date_fmt: &Format,
    width_map: &mut HashMap<u16, usize>,
) {
    add_string_column(row, 0, &value.id.to_string(), sheet, width_map);
    add_date_column(row, 1, &value.created, sheet, width_map, date_fmt);
    add_string_column(row, 2, &value.first_name, sheet, width_map);
    add_string_column(row, 3, &value.last_name, sheet, width_map);
    add_string_column(row, 4, &value.street, sheet, width_map);
    add_string_column(row, 5, &value.city, sheet, width_map);
    add_string_column(row, 6, &value.email, sheet, width_map);
    add_opt_string_column(row, 7, &value.phone, sheet, width_map);
    add_bool_column(row, 8, &value.member, sheet, width_map);
    add_string_column(
        row,
        9,
        &event.cost(value.member).to_euro(),
        sheet,
        width_map,
    );
    add_string_column(row, 10, &value.payment_id, sheet, width_map);
    add_bool_column(row, 11, &value.payed, sheet, width_map);
    add_opt_string_column(row, 12, &value.comment, sheet, width_map);

    let _ = sheet.set_row(row, FONT_SIZE, None);
}

fn add_bool_column(
    row: u32,
    column: u16,
    data: &bool,
    sheet: &mut Worksheet,
    width_map: &mut HashMap<u16, usize>,
) {
    add_string_column(
        row,
        column,
        match data {
            true => "Y",
            false => "N",
        },
        sheet,
        width_map,
    );
}

fn add_opt_string_column(
    row: u32,
    column: u16,
    data: &Option<String>,
    sheet: &mut Worksheet,
    width_map: &mut HashMap<u16, usize>,
) {
    if let Some(data) = data {
        add_string_column(row, column, data, sheet, width_map);
    }
}

fn add_string_column(
    row: u32,
    column: u16,
    data: &str,
    sheet: &mut Worksheet,
    width_map: &mut HashMap<u16, usize>,
) {
    let _ = sheet.write_string(row + 1, column, data, None);
    set_new_max_width(column, data.len(), width_map);
}

fn add_date_column(
    row: u32,
    column: u16,
    date: &DateTime<Utc>,
    sheet: &mut Worksheet,
    width_map: &mut HashMap<u16, usize>,
    date_fmt: &Format,
) {
    let d = xlsxwriter::DateTime::new(
        date.year() as i16,
        date.month() as i8,
        date.day() as i8,
        date.hour() as i8,
        date.minute() as i8,
        date.second() as f64,
    );

    let _ = sheet.write_datetime(row + 1, column, &d, Some(date_fmt));
    set_new_max_width(column, 26, width_map);
}

fn create_headers(
    sheet: &mut Worksheet,
    bold_fmt: &Format,
    width_map: &mut HashMap<u16, usize>,
) {
    let _ = sheet.write_string(0, 0, "Id", Some(bold_fmt));
    let _ = sheet.write_string(0, 1, "Buchungsdatum", Some(bold_fmt));
    let _ = sheet.write_string(0, 2, "Vorname", Some(bold_fmt));
    let _ = sheet.write_string(0, 3, "Nachname", Some(bold_fmt));
    let _ = sheet.write_string(0, 4, "Straße & Nr", Some(bold_fmt));
    let _ = sheet.write_string(0, 5, "PLZ & Ort", Some(bold_fmt));
    let _ = sheet.write_string(0, 6, "Email", Some(bold_fmt));
    let _ = sheet.write_string(0, 7, "Telefon", Some(bold_fmt));
    let _ = sheet.write_string(0, 8, "Mitglied", Some(bold_fmt));
    let _ = sheet.write_string(0, 9, "Betrag", Some(bold_fmt));
    let _ = sheet.write_string(0, 10, "Buchungsnr", Some(bold_fmt));
    let _ = sheet.write_string(0, 11, "Bezahlt", Some(bold_fmt));
    let _ = sheet.write_string(0, 12, "Kommentar", Some(bold_fmt));

    set_new_max_width(0, "Id".len(), width_map);
    set_new_max_width(1, "Buchungsdatum".len(), width_map);
    set_new_max_width(2, "Vorname".len(), width_map);
    set_new_max_width(3, "Nachname".len(), width_map);
    set_new_max_width(4, "Straße & Nr".len(), width_map);
    set_new_max_width(5, "PLZ & Ort".len(), width_map);
    set_new_max_width(6, "Email".len(), width_map);
    set_new_max_width(7, "Telefon".len(), width_map);
    set_new_max_width(8, "Mitglied".len(), width_map);
    set_new_max_width(9, "Betrag".len(), width_map);
    set_new_max_width(10, "Buchungsnr".len(), width_map);
    set_new_max_width(11, "Bezahlt".len(), width_map);
    set_new_max_width(12, "Kommentar".len(), width_map);
}

fn set_new_max_width(col: u16, new: usize, width_map: &mut HashMap<u16, usize>) {
    match width_map.get(&col) {
        Some(max) => {
            if new > *max {
                width_map.insert(col, new);
            }
        }
        None => {
            width_map.insert(col, new);
        }
    };
}
