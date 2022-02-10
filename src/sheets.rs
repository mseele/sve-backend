use crate::models::{Event, EventBooking};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use chrono_tz::Europe::Berlin;
use google_sheets4::{api::ValueRange, Sheets};
use steel_cent::formatting::{format, france_style as euro_style};
use yup_oauth2::ServiceAccountKey;

const REQUIRED_HEADERS: [&str; 11] = [
    "buchungsdatum",
    "vorname",
    "nachname",
    "straße & nr",
    "plz & ort",
    "email",
    "telefon",
    "sve-mitglied",
    "betrag",
    "bezahlt",
    "kommentar",
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

pub async fn save_booking(booking: &EventBooking, event: &Event) -> Result<()> {
    let hub = sheets_hub().await?;
    let sheet_title = get_sheet_title(&hub, event).await?;
    let (insert_index, headers_indices) = resolve_indices(&hub, event, &sheet_title).await?;
    insert(
        &hub,
        booking,
        event,
        &sheet_title,
        insert_index,
        headers_indices,
    )
    .await?;
    Ok(())
}

async fn insert(
    hub: &Sheets,
    booking: &EventBooking,
    event: &Event,
    sheet_title: &str,
    insert_index: usize,
    headers_indices: Vec<usize>,
) -> Result<()> {
    let current_date_time = Utc::now()
        .with_timezone(&Berlin)
        .format("%d.%m.%Y %H:%M:%S")
        .to_string();
    let phone_number = booking.phone.clone().map_or(String::new(), |v| {
        let mut value = String::from("'");
        value.push_str(&v);
        value
    });
    let member = match booking.is_member() {
        true => String::from("J"),
        false => String::from("N"),
    };
    let cost = format(euro_style(), &booking.cost(event));
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
        cost,
        String::from("N"),
        comments,
    ];
    sort_by_indices(&mut values, headers_indices);

    hub.spreadsheets()
        .values_update(
            ValueRange {
                values: Some(vec![values]),
                ..Default::default()
            },
            &event.sheet_id,
            format!("'{0}'!B{1}:L{1}", sheet_title, insert_index,).as_str(),
        )
        .value_input_option("USER_ENTERED")
        .doit()
        .await?;

    Ok(())
}

async fn resolve_indices(
    hub: &Sheets,
    event: &Event,
    sheet_title: &str,
) -> Result<(usize, Vec<usize>)> {
    let (_, value_range) = hub
        .spreadsheets()
        .values_get(
            &event.sheet_id,
            format!("'{}'!B1:L1000", sheet_title).as_str(),
        )
        .doit()
        .await?;

    match value_range.values {
        Some(values) => {
            // verify all headers are available and store their indices
            let headers_indices = get_header_indices(&values[0]).with_context(|| {
                format!(
                    "Headers are missing in sheet '{}' of spreadsheet '{}'",
                    sheet_title, &event.sheet_id,
                )
            })?;

            // first empty row is the value length +1
            Ok((values.len() + 1, headers_indices))
        }
        None => bail!(
            "Found no values in sheet '{}' of spreadsheet '{}'",
            sheet_title,
            &event.sheet_id,
        ),
    }
}

async fn get_sheet_title(hub: &Sheets, event: &Event) -> Result<String> {
    let (_, spreadsheet) = hub
        .spreadsheets()
        .get(&event.sheet_id)
        .param("fields", "sheets(properties(sheetId,title))")
        .doit()
        .await?;

    let title = spreadsheet.sheets.and_then(|sheets| {
        sheets.into_iter().find_map(|sheet| {
            sheet.properties.and_then(|properties| {
                if let Some(sheet_id) = properties.sheet_id {
                    let sheet_id: i64 = sheet_id.into();
                    if sheet_id == event.gid {
                        return properties.title;
                    }
                }
                None
            })
        })
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

    #[test]
    fn correct_header_indices() {
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
        ];
        let indices = get_header_indices(&values);
        assert_eq!(indices.is_ok(), true);
        assert_eq!(indices.unwrap(), &[7, 0, 1, 2, 5, 3, 4, 6, 8, 10, 9]);
    }

    #[test]
    fn correct_order() {
        let indices = vec![0, 3, 2, 1];
        let mut data = vec!["a", "b", "c", "d"];
        sort_by_indices(&mut data, indices);
        assert_eq!(data, &["a", "d", "c", "b"]);
    }
}
