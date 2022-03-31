use std::collections::HashMap;

use crate::models::{Event, EventBooking, FromEuro, ToEuro, VerifyPaymentBookingRecord};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use chrono_tz::Europe::Berlin;
use google_sheets4::{
    api::{BatchUpdateValuesRequest, SheetProperties, ValueRange},
    Sheets,
};
use yup_oauth2::ServiceAccountKey;

const REQUIRED_HEADERS: [&str; 12] = [
    "buchungsdatum",
    "vorname",
    "nachname",
    "straße & nr",
    "plz & ort",
    "email",
    "telefon",
    "sve-mitglied",
    "betrag",
    "buchungsnr",
    "bezahlt",
    "kommentar",
];

const VORNAME: usize = 1;
const NACHNAME: usize = 2;
const STRASSE_NR: usize = 3;
const PLZ_ORT: usize = 4;
const EMAIL: usize = 5;
const TELEFON: usize = 6;
const MITGLIED: usize = 7;
const BETRAG: usize = 8;
const BUCHUNGSNR: usize = 9;
const BEZAHLT: usize = 10;

const REQUIRED_PREBOOKING_HEADERS: [usize; 7] = [
    VORNAME, NACHNAME, STRASSE_NR, PLZ_ORT, EMAIL, TELEFON, MITGLIED,
];

async fn sheets_hub() -> Result<Sheets> {
    let secret: ServiceAccountKey =
        serde_json::from_str(crate::CREDENTIALS).with_context(|| "Error loading credentials")?;

    let auth = yup_oauth2::ServiceAccountAuthenticator::builder(secret)
        .build()
        .await?;

    let hub = Sheets::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

    Ok(hub)
}

pub async fn save_booking(
    booking: &EventBooking,
    event: &Event,
    booking_number: &String,
) -> Result<()> {
    let hub = sheets_hub().await?;
    let sheet_title = get_sheet_title(&hub, event).await?;
    let values = get_values(&hub, &event.sheet_id, &sheet_title).await?;
    match values {
        Some(values) => {
            // verify all headers are available and store their indices
            let header_indices = get_header_indices(&values[0]).with_context(|| {
                format!(
                    "Headers are missing in sheet '{}' of spreadsheet '{}'",
                    sheet_title, &event.sheet_id,
                )
            })?;

            // first empty row is the value length +1
            let insert_index = values.len() + 1;

            insert(
                &hub,
                booking,
                event,
                booking_number,
                &sheet_title,
                insert_index,
                header_indices,
            )
            .await?;

            Ok(())
        }
        None => bail!(
            "Found no values in sheet '{}' of spreadsheet '{}'",
            sheet_title,
            &event.sheet_id,
        ),
    }
}

/// Result of the `detect_booking` function
pub enum BookingDetection {
    Booked,
    NotBooked,
}

/// Returns `BookingDetection::Booked` if there is already a dataset
/// in the spreadsheet for the given booking and
/// `BookingDetection::NotBooked` if there is no dataset in the spreadsheet.
pub async fn detect_booking(booking: &EventBooking, event: &Event) -> Result<BookingDetection> {
    let hub = sheets_hub().await?;
    let sheet_title = get_sheet_title(&hub, event).await?;
    let values = get_values(&hub, &event.sheet_id, &sheet_title).await?;

    match values {
        Some(values) => {
            let header_indices = get_header_indices(&values[0]).with_context(|| {
                format!(
                    "Headers are missing in sheet '{}' of spreadsheet '{}'",
                    sheet_title, &event.sheet_id,
                )
            })?;
            // collect the subset of prebooking header indices
            let prebooking_header_indices = header_indices
                .iter()
                .filter(|i| REQUIRED_PREBOOKING_HEADERS.contains(i))
                .map(|i| *i)
                .collect::<Vec<_>>();
            // generate booking values and filter by prebooking headers
            let prebooking_values = filter_by_indices(
                into_values(booking, event, &String::from(""), header_indices, false),
                &prebooking_header_indices,
            );
            // check prebooking values for existance
            match values.into_iter().skip(1).find(|row| {
                vec_compare(
                    &filter_by_indices(row.to_vec(), &prebooking_header_indices),
                    &prebooking_values,
                )
            }) {
                Some(_) => Ok(BookingDetection::Booked),
                None => Ok(BookingDetection::NotBooked),
            }
        }
        None => Ok(BookingDetection::NotBooked),
    }
}

pub async fn get_bookings_to_verify_payment(
    sheet_id: &str,
) -> Result<Vec<VerifyPaymentBookingRecord>> {
    let hub = sheets_hub().await?;
    let sheet_properties = get_sheet_properties(&hub, sheet_id).await?;
    let mut records = Vec::new();
    for properties in sheet_properties {
        if let Some(title) = properties.title {
            let values = get_values(&hub, sheet_id, &title).await?;
            if let Some(values) = values {
                if let Ok(header_indices) = get_header_indices(&values[0]) {
                    // calculate the payed column as alphabetic character
                    let payed_column = header_indices
                        .iter()
                        .position(|i| i == &BEZAHLT)
                        .expect("Index {BEZAHLT} is inside header_indices")
                        as u32;
                    let payed_column = char::from_u32(
                        0x0041 + 1 + payed_column,
                    ).ok_or_else(|| anyhow!("Could not convert index of column {payed_column} into alphanumeric character"))?;

                    // convert bookings
                    for (index, row) in values.into_iter().enumerate().skip(1) {
                        let mut values: HashMap<_, _> = row.into_iter().enumerate().collect();

                        let cost = values
                            .remove(&BETRAG)
                            .expect("Index {BETRAG} is inside header_indices");
                        let cost = cost.trim().from_euro_with_symbol().with_context(|| {
                            format!("Could not parse '{cost}' in sheet {title} at row {index}")
                        })?;

                        let booking_number = values
                            .remove(&BUCHUNGSNR)
                            .expect("Index {BUCHUNGSNR} is inside header_indices")
                            .trim()
                            .into();
                        let payed_already = values
                            .remove(&BEZAHLT)
                            .expect("Index {BEZAHLT} is inside header_indices")
                            .to_uppercase()
                            .trim()
                            .eq("J");
                        let update_cell = format!("{}{}", payed_column, index + 1);

                        records.push(VerifyPaymentBookingRecord::new(
                            title.clone(),
                            update_cell,
                            cost,
                            booking_number,
                            payed_already,
                        ));
                    }
                }
            }
        }
    }

    Ok(records)
}

pub async fn mark_as_payed(
    sheet_id: &str,
    bookings: Vec<VerifyPaymentBookingRecord>,
) -> Result<()> {
    let hub = sheets_hub().await?;

    let request = BatchUpdateValuesRequest {
        data: Some(
            bookings
                .into_iter()
                .map(|record| record.into_value_range())
                .collect(),
        ),
        value_input_option: Some("USER_ENTERED".into()),
        ..Default::default()
    };

    hub.spreadsheets()
        .values_batch_update(request, sheet_id)
        .doit()
        .await?;

    Ok(())
}

fn filter_by_indices(values: Vec<String>, indices: &[usize]) -> Vec<String> {
    values
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| {
            if indices.contains(&index) {
                return Some(value);
            }
            return None;
        })
        .collect::<Vec<_>>()
}

fn vec_compare(va: &[String], vb: &[String]) -> bool {
    (va.len() == vb.len()) &&  // zip stops at the shortest
     va.iter()
       .zip(vb)
       .all(|(a,b)| a.to_lowercase().trim().eq(b.to_lowercase().trim()))
}

fn into_values(
    booking: &EventBooking,
    event: &Event,
    booking_number: &String,
    header_indices: Vec<usize>,
    prefix_phone_number: bool,
) -> Vec<String> {
    let current_date_time = Utc::now()
        .with_timezone(&Berlin)
        .format("%d.%m.%Y %H:%M:%S")
        .to_string();
    let phone_number = match &booking.phone {
        Some(phone_number) if phone_number.trim().len() > 0 => match prefix_phone_number {
            true => {
                let mut value = String::from("'");
                value.push_str(phone_number.trim());
                value
            }
            false => phone_number.trim().into(),
        },
        Some(_) | None => String::from(""),
    };
    let member = match booking.is_member() {
        true => String::from("J"),
        false => String::from("N"),
    };
    let comments = booking.comments.clone().unwrap_or(String::new());
    let mut values = vec![
        current_date_time,
        booking.first_name.clone(),
        booking.last_name.clone(),
        booking.street.clone(),
        booking.city.clone(),
        booking.email.clone(),
        phone_number,
        member,
        booking.cost(event).to_euro_without_symbol(),
        booking_number.clone(),
        String::from("N"),
        comments,
    ];
    sort_by_indices(&mut values, header_indices);

    values
}

async fn insert(
    hub: &Sheets,
    booking: &EventBooking,
    event: &Event,
    booking_number: &String,
    sheet_title: &str,
    insert_index: usize,
    header_indices: Vec<usize>,
) -> Result<()> {
    let values = into_values(booking, event, booking_number, header_indices, true);
    hub.spreadsheets()
        .values_update(
            ValueRange {
                values: Some(vec![values]),
                ..Default::default()
            },
            &event.sheet_id,
            format!("'{0}'!B{1}:M{1}", sheet_title, insert_index).as_str(),
        )
        .value_input_option("USER_ENTERED")
        .doit()
        .await?;

    Ok(())
}

async fn get_values(
    hub: &Sheets,
    sheet_id: &str,
    sheet_title: &str,
) -> Result<Option<Vec<Vec<String>>>> {
    let (_, value_range) = hub
        .spreadsheets()
        .values_get(sheet_id, format!("'{}'!B1:M1000", sheet_title).as_str())
        .doit()
        .await?;

    Ok(value_range.values)
}

async fn get_sheet_title(hub: &Sheets, event: &Event) -> Result<String> {
    let title = get_sheet_properties(hub, &event.sheet_id)
        .await?
        .into_iter()
        .find_map(|properties| {
            if let Some(sheet_id) = properties.sheet_id {
                let sheet_id: i64 = sheet_id.into();
                if sheet_id == event.gid {
                    return properties.title;
                }
            }
            None
        });

    if let Some(value) = title {
        Ok(value)
    } else {
        bail!(
            "Sheet with sheet_id {} does not exist in spreadsheet {}",
            event.gid,
            event.sheet_id
        )
    }
}

async fn get_sheet_properties(hub: &Sheets, sheet_id: &str) -> Result<Vec<SheetProperties>> {
    let (_, spreadsheet) = hub
        .spreadsheets()
        .get(sheet_id)
        .param("fields", "sheets(properties(sheetId,title))")
        .doit()
        .await?;

    let properties = spreadsheet
        .sheets
        .ok_or_else(|| anyhow!("Could not load properties from spreadsheet {} ", sheet_id))?
        .into_iter()
        .filter_map(|sheet| sheet.properties)
        .collect::<Vec<_>>();

    Ok(properties)
}

fn get_header_indices(values: &Vec<String>) -> Result<Vec<usize>> {
    let mut indices = vec![0; REQUIRED_HEADERS.len()];
    for (header_index, required_header) in REQUIRED_HEADERS.into_iter().enumerate() {
        let index = values
            .iter()
            .position(|header| header.to_lowercase() == required_header)
            .with_context(|| {
                format!(
                    "Values '{:?}' do not contain required header '{}'",
                    values, required_header
                )
            })?;
        indices[index] = header_index;
    }
    Ok(indices)
}

fn sort_by_indices<T>(data: &mut [T], mut indices: Vec<usize>) {
    for idx in 0..data.len() {
        if indices[idx] != idx {
            let mut current_idx = idx;
            loop {
                let target_idx = indices[current_idx];
                indices[current_idx] = current_idx;
                if indices[target_idx] == target_idx {
                    break;
                }
                data.swap(current_idx, target_idx);
                current_idx = target_idx;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_vec_compare() {
        // success
        assert_eq!(
            vec_compare(
                &[String::from("a"), String::from("b"), String::from("c")],
                &[String::from("a"), String::from("b"), String::from("c")]
            ),
            true
        );
        assert_eq!(
            vec_compare(
                &[
                    String::from(" aAa "),
                    String::from("Bä"),
                    String::from("  c")
                ],
                &[
                    String::from(" AaA "),
                    String::from("bÄ  "),
                    String::from("C")
                ]
            ),
            true
        );

        // failure
        assert_eq!(
            vec_compare(
                &[String::from("1")],
                &[String::from("1"), String::from("2")]
            ),
            false
        );
        assert_eq!(
            vec_compare(
                &[String::from("a"), String::from("b"), String::from("c")],
                &[String::from("a"), String::from("c"), String::from("b")]
            ),
            false
        );
    }

    #[test]
    fn test_get_header_indices() {
        let values = vec![
            String::from("SVE-Mitglied"),
            String::from("Buchungsdatum"),
            String::from("Vorname"),
            String::from("Nachname"),
            String::from("Email"),
            String::from("Straße & Nr"),
            String::from("PLZ & Ort"),
            String::from("Telefon"),
            String::from("Betrag"),
            String::from("Kommentar"),
            String::from("Bezahlt"),
            String::from("Buchungsnr"),
        ];
        let indices = get_header_indices(&values);
        assert_eq!(indices.is_ok(), true);
        assert_eq!(indices.unwrap(), &[7, 0, 1, 2, 5, 3, 4, 6, 8, 11, 10, 9]);
    }

    #[test]
    fn test_sort_by_indices() {
        let indices = vec![0, 3, 2, 1];
        let mut data = vec!["a", "b", "c", "d"];
        sort_by_indices(&mut data, indices);
        assert_eq!(data, &["a", "d", "c", "b"]);
    }
}
