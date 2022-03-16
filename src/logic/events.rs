use crate::email;
use crate::models::{
    BookingResponse, EventBooking, EventCounter, EventType, Subscription, ToEuro,
    VerifyPaymentBookingRecord, VerifyPaymentResult,
};
use crate::models::{Event, PartialEvent};
use crate::sheets::{self, BookingDetection};
use crate::store::{self, BookingResult, GouthInterceptor};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Locale, Utc};
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use log::{error, info, warn};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::str::from_utf8;
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

const MESSAGE_FAIL: &str =
    "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal.";

pub async fn get_events(all: Option<bool>, beta: Option<bool>) -> Result<Vec<Event>> {
    let mut client = store::get_client().await?;
    let mut events = get_and_filter_events(&mut client, all, beta).await?;

    // sort events
    events.sort_unstable_by(|a, b| {
        let is_a_booked_up = a.is_booked_up();
        let is_b_booked_up = b.is_booked_up();
        if is_a_booked_up == is_b_booked_up {
            return a.sort_index.cmp(&b.sort_index);
        }
        return is_a_booked_up.cmp(&is_b_booked_up);
    });

    Ok(events)
}

pub async fn get_event_counters() -> Result<Vec<EventCounter>> {
    let mut client = store::get_client().await?;
    let event_counters = create_event_counters(&mut client).await?;

    Ok(event_counters)
}

pub async fn booking(booking: EventBooking) -> BookingResponse {
    match do_booking(booking).await {
        Ok(response) => response,
        Err(e) => {
            error!("Booking failed: {}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub async fn prebooking(hash: String) -> BookingResponse {
    match do_prebooking(hash).await {
        Ok(response) => response,
        Err(e) => {
            error!("Prebooking failed: {}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub async fn update(partial_event: PartialEvent) -> Result<Event> {
    let mut client = store::get_client().await?;
    let result = store::write_event(&mut client, partial_event).await?;

    Ok(result)
}

pub async fn delete(partial_event: PartialEvent) -> Result<()> {
    let mut client = store::get_client().await?;
    store::delete_event(&mut client, &partial_event.id).await?;

    Ok(())
}

pub async fn verify_payments(sheet_id: String, csv: String) -> Result<Vec<VerifyPaymentResult>> {
    let bytes = base64::decode(&csv)
        .with_context(|| format!("Error decoding the cvs content: {}", &csv))?;
    let csv = from_utf8(&bytes).with_context(|| {
        format!(
            "Error converting the decoded csv content {} into a string slice",
            &csv
        )
    })?;
    let mut bookings = sheets::get_bookings_to_verify_payment(&sheet_id).await?;
    let (verified_payment_bookings, result) = compare_csv_with_bookings(csv, &mut bookings)?;
    if verified_payment_bookings.len() > 0 {
        sheets::mark_as_payed(&sheet_id, verified_payment_bookings).await?;
    }
    Ok(result)
}

async fn get_and_filter_events(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    all: Option<bool>,
    beta: Option<bool>,
) -> Result<Vec<Event>> {
    let events = store::get_events(client).await?;

    let events = events
        .into_iter()
        .filter(|event| {
            // keep event if all is true
            if all.unwrap_or(false) {
                return true;
            }
            // keep event if it is visible
            if event.visible {
                // and beta is the same state than event
                if let Some(beta) = beta {
                    return beta == event.beta;
                }
                // and beta is None
                return true;
            }
            return false;
        })
        .collect::<Vec<_>>();

    Ok(events)
}

async fn create_event_counters(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
) -> Result<Vec<EventCounter>> {
    let event_counters = get_and_filter_events(client, None, None)
        .await?
        .into_iter()
        .map(|event| event.into())
        .collect::<Vec<EventCounter>>();

    Ok(event_counters)
}

async fn do_booking(booking: EventBooking) -> Result<BookingResponse> {
    let mut client = store::get_client().await?;
    Ok(book_event(&mut client, booking).await?)
}

async fn book_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    booking: EventBooking,
) -> Result<BookingResponse> {
    let booking_result = &store::book_event(client, &booking.event_id).await?;
    let result = match booking_result {
        BookingResult::Booked(event, booking_number)
        | BookingResult::WaitingList(event, booking_number) => {
            sheets::save_booking(&booking, event, booking_number).await?;
            subscribe_to_updates(client, &booking, event).await?;
            send_mail(&booking, event, booking_result).await?;
            info!("Booking of Event {} was successfull", booking.event_id);
            let message;
            if let BookingResult::Booked(_, _) = booking_result {
                message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail.";
            } else {
                message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden.";
            }
            BookingResponse::success(message, create_event_counters(client).await?)
        }
        BookingResult::BookedOut => {
            error!(
                "Booking failed because Event ({}) was overbooked.",
                booking.event_id
            );
            BookingResponse::failure(MESSAGE_FAIL)
        }
    };

    Ok(result)
}

async fn do_prebooking(hash: String) -> Result<BookingResponse> {
    let bytes = base64::decode(&hash)
        .with_context(|| format!("Error decoding the prebooking hash {}", &hash))?;
    let decoded = from_utf8(&bytes).with_context(|| {
        format!(
            "Error converting the decoded prebooking hash {} into a string slice",
            &hash
        )
    })?;
    let splitted = decoded.split('#').collect::<Vec<_>>();

    // fail if the length is not correct
    if splitted.len() != 8 {
        bail!(
            "Booking failed beacuse spitted prebooking hash ({}) has an invalid length: {}",
            decoded,
            splitted.len()
        );
    }

    let booking = EventBooking::new(
        splitted[0].into(),
        splitted[1].into(),
        splitted[2].into(),
        splitted[3].into(),
        splitted[4].into(),
        splitted[5].into(),
        Some(splitted[6].into()),
        Some("J".eq(splitted[7])),
        Some(false),
        Some(String::from("Pre-Booking")),
    );

    let mut client = store::get_client().await?;
    let event = store::get_event(&mut client, &booking.event_id).await?;

    // prebooking is only available for beta events
    if !event.beta {
        warn!("Prebooking has ended for booking {:?}", booking);
        return Ok(BookingResponse::failure(
            "Der Buchungslink ist nicht mehr gültig da die Frühbuchungsphase zu Ende ist.",
        ));
    }

    // check if prebooking has been used already
    let result = sheets::detect_booking(&booking, &event).await?;
    if let BookingDetection::Booked = result {
        warn!(
            "Prebooking link data has been detected and invalidated for booking {:?}",
            booking
        );
        return Ok(BookingResponse::failure(
            "Der Buchungslink wurde schon benutzt und ist daher ungültig.",
        ));
    }

    Ok(book_event(&mut client, booking).await?)
}

async fn subscribe_to_updates(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    booking: &EventBooking,
    event: &Event,
) -> Result<()> {
    // only subscribe to updates if updates field is true
    if booking.updates.unwrap_or(false) == false {
        return Ok(());
    }
    // TODO: try to get rid of clone
    let subscription =
        Subscription::new(booking.email.clone(), vec![event.event_type.clone().into()]);
    super::news::subscribe_to_news(client, subscription, false).await?;

    Ok(())
}

async fn send_mail(
    booking: &EventBooking,
    event: &Event,
    booking_result: &BookingResult,
) -> Result<()> {
    let email_account = match &event.alt_email_address {
        Some(email_address) => email::get_account_by_address(email_address),
        None => email::get_account_by_type(event.event_type.into()),
    }?;
    let subject_prefix = format!(
        "[{}@SVE]",
        match event.event_type {
            EventType::Fitness => "Fitness",
            EventType::Events => "Events",
        }
    );
    let subject;
    let template;
    let booking_number;
    if let BookingResult::Booked(_, b_nr) = booking_result {
        subject = format!("{} Bestätigung Buchung", subject_prefix);
        template = &event.booking_template;
        booking_number = Some(b_nr);
    } else {
        subject = format!("{} Bestätigung Warteliste", subject_prefix);
        template = &event.waiting_template;
        booking_number = None;
    }

    let message = email_account
        .new_message()?
        .to(booking.email.parse()?)
        .bcc(email_account.mailbox()?)
        .subject(subject)
        .body(create_body(template, booking, event, booking_number))?;

    email::send_message(&email_account, message).await?;

    Ok(())
}

fn create_body(
    template: &str,
    booking: &EventBooking,
    event: &Event,
    booking_number: Option<&String>,
) -> String {
    let mut body = template
        .replace("${firstname}", booking.first_name.trim())
        .replace("${lastname}", booking.last_name.trim())
        .replace("${name}", event.name.trim())
        .replace("${location}", &event.location)
        .replace("${price}", &booking.cost_as_string(event))
        .replace("${dates}", &format_dates(&event));
    if let Some(booking_number) = booking_number {
        body = body.replace("${booking_number}", booking_number);
    }
    body = replace_payday(body, &event);
    if booking.updates.unwrap_or(false) {
        body.push_str(
            format!(
                "

PS: Ab sofort erhältst Du automatisch eine E-Mail, sobald neue {} online sind.
{}",
                match event.event_type {
                    EventType::Fitness => "Kursangebote",
                    EventType::Events => "Events",
                },
                super::news::UNSUBSCRIBE_MESSAGE
            )
            .as_str(),
        )
    }
    body
}

fn format_dates(event: &Event) -> String {
    event
        .dates
        .iter()
        .map(|d| {
            DateTime::<Utc>::from_utc(*d, Utc)
                .format_localized("- %a, %d. %B %Y, %H:%M Uhr", Locale::de_DE)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn replace_payday(body: String, event: &Event) -> String {
    if let Some(first_date) = event.dates.first() {
        let mut payday_replace_str = "${payday}";
        let mut days = 14;
        let payday_regex = Regex::new(r"\$\{payday:(\d+)\}").expect("regex is valid");
        if let Some(find_result) = payday_regex.find(&body) {
            payday_replace_str = &body[find_result.start()..find_result.end()];
            if let Some(captures) = payday_regex.captures(&body) {
                days = captures
                    .get(1)
                    .map_or("", |m| m.as_str())
                    .parse::<i32>()
                    .expect("group is an integer");
            }
        }
        let mut payday = *first_date - Duration::days(days.into());
        let tomorrow = Utc::now().naive_utc() + Duration::days(1);
        if payday < tomorrow {
            payday = tomorrow
        }

        return body.replace(
            payday_replace_str,
            &DateTime::<Utc>::from_utc(payday, Utc)
                .format_localized("%d. %B", Locale::de_DE)
                .to_string(),
        );
    }

    body
}

#[derive(Debug, Deserialize)]
struct PaymentRecord {
    #[serde(rename = "Buchungstag")]
    _date: String,
    #[serde(rename = "Valuta")]
    _valuta: String,
    #[serde(rename = "Textschlüssel")]
    _textkey: String,
    #[serde(rename = "Primanota")]
    primanota: String,
    #[serde(rename = "Zahlungsempfänger")]
    payee: String,
    #[serde(rename = "ZahlungsempfängerKto")]
    _payee_account: String,
    #[serde(rename = "ZahlungsempfängerIBAN")]
    payee_iban: String,
    #[serde(rename = "ZahlungsempfängerBLZ")]
    _payee_blz: String,
    #[serde(rename = "ZahlungsempfängerBIC")]
    _payee_bic: String,
    #[serde(rename = "Vorgang/Verwendungszweck")]
    purpose: String,
    #[serde(rename = "Kundenreferenz")]
    _customer_reference: String,
    #[serde(rename = "Währung")]
    _currency: String,
    #[serde(rename = "Umsatz", deserialize_with = "deserialize_float_with_comma")]
    volumne: f64,
    #[serde(rename = "Soll/Haben")]
    debit_credit: String,
}

impl PaymentRecord {
    fn volumne(&self) -> f64 {
        match self.debit_credit.as_str() {
            "H" => self.volumne,
            _ => -self.volumne,
        }
    }
}

fn deserialize_float_with_comma<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    value
        .replace(",", ".")
        .parse::<f64>()
        .map_err(serde::de::Error::custom)
}

fn compare_csv_with_bookings(
    csv: &str,
    bookings: &mut Vec<VerifyPaymentBookingRecord>,
) -> Result<(Vec<VerifyPaymentBookingRecord>, Vec<VerifyPaymentResult>)> {
    let csv_records_prefix = "Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben";
    let csv_records_suffix = ";;;;;;;;;;;;;";
    let start = csv
        .find(csv_records_prefix)
        .ok_or_else(|| anyhow!("Found no title row in uploaded csv:\n\n{}", csv))?;
    let end = csv[start..].find(csv_records_suffix).ok_or_else(|| {
        anyhow!(
            "Found no end sequence in uploaded csv:\n\n{}",
            &csv[start..]
        )
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(csv[start..(start + end)].as_bytes());
    let mut verified_payment_bookings = Vec::new();
    let mut payment_bookings_with_errors = BTreeMap::new();
    let mut non_matching_payment_records = Vec::new();

    for result in reader.deserialize() {
        let record: PaymentRecord = result?;
        let position = bookings.iter().position(|booking| {
            booking.booking_number.len() > 0 && record.purpose.contains(&booking.booking_number)
        });
        if let Some(index) = position {
            let booking = bookings.remove(index);

            if booking.payed_already {
                payment_bookings_with_errors
                    .entry(booking.booking_number.clone())
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Doppelt bezahlt: Buchung war schon als bezahlt markiert"
                    ));
            }

            let record_volumne = record.volumne().to_euro_string();
            let booking_cost = booking.cost.to_euro_string();
            if !record_volumne.eq(&booking_cost) {
                payment_bookings_with_errors
                    .entry(booking.booking_number.clone())
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Betrag falsch: erwartet {booking_cost} != überwiesen {record_volumne}"
                    ));
            }

            if !payment_bookings_with_errors.contains_key(&booking.booking_number) {
                verified_payment_bookings.push(booking);
            }
        } else {
            non_matching_payment_records.push(format!(
                "{} / {} / {} / {} / {}",
                record.primanota,
                record.payee,
                record.payee_iban,
                record.purpose,
                record.volumne().to_euro_string()
            ));
        }
    }
    verified_payment_bookings.sort_unstable_by(|a, b| a.booking_number.cmp(&b.booking_number));

    let mut compare_result = Vec::new();

    // first group are the successfull matches
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} bezahlte {}",
            verified_payment_bookings.len(),
            match verified_payment_bookings.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        verified_payment_bookings
            .iter()
            .map(|booking| booking.booking_number.clone())
            .collect::<Vec<_>>(),
    ));

    // second group are the matches with errors
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} {} mit Problemen",
            payment_bookings_with_errors.len(),
            match payment_bookings_with_errors.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        payment_bookings_with_errors
            .into_iter()
            .map(|(booking_number, mut errors)| {
                errors.insert(0, booking_number);
                errors.join(" / ")
            })
            .collect::<Vec<_>>(),
    ));

    // third group are all non matching payment records
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} nicht erkannte {}",
            non_matching_payment_records.len(),
            match non_matching_payment_records.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        non_matching_payment_records,
    ));

    Ok((verified_payment_bookings, compare_result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_create_body() {
        let booking_member = EventBooking::new(
            String::from("id"),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("Haupstraße 1"),
            String::from("72184 Eutingen"),
            String::from("max@mustermann.de"),
            None,
            Some(true),
            None,
            None,
        );
        let booking_non_member = EventBooking::new(
            String::from("id"),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("Haupstraße 1"),
            String::from("72184 Eutingen"),
            String::from("max@mustermann.de"),
            None,
            None,
            None,
            None,
        );
        let event = Event::new(
            String::from("id"),
            String::from("sheet_id"),
            0,
            EventType::Fitness,
            String::from("FitForFun"),
            0,
            true,
            false,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            vec![
                NaiveDate::from_ymd(2022, 3, 7).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 8).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 9).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 10).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 11).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 12).and_hms(19, 00, 00),
                NaiveDate::from_ymd(2022, 3, 13).and_hms(19, 00, 00),
            ],
            None,
            0,
            0,
            0,
            5.0,
            10.0,
            0,
            0,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        );

        assert_eq!(
            create_body(
                "${firstname} ${lastname} ${name} ${location} ${price} ${payday:0} ${booking_number}
${dates}",
                &booking_member,
                &event,
                None
            ),
            format!(
                "Max Mustermann FitForFun Turn- & Festhalle Eutingen 5,00\u{a0}€ {} ${{booking_number}}
- Mo, 07. März 2022, 19:00 Uhr
- Di, 08. März 2022, 19:00 Uhr
- Mi, 09. März 2022, 19:00 Uhr
- Do, 10. März 2022, 19:00 Uhr
- Fr, 11. März 2022, 19:00 Uhr
- Sa, 12. März 2022, 19:00 Uhr
- So, 13. März 2022, 19:00 Uhr",
                format_payday(Utc::now() + Duration::days(1))
            )
        );
        assert_eq!(
            create_body(
                "${firstname} ${lastname} ${name} ${location} ${price} ${payday:0} ${booking_number}
${dates}",
                &booking_non_member,
                &event,
                Some(&String::from("22-1012"))
            ),
            format!(
                "Max Mustermann FitForFun Turn- & Festhalle Eutingen 10,00\u{a0}€ {} 22-1012
- Mo, 07. März 2022, 19:00 Uhr
- Di, 08. März 2022, 19:00 Uhr
- Mi, 09. März 2022, 19:00 Uhr
- Do, 10. März 2022, 19:00 Uhr
- Fr, 11. März 2022, 19:00 Uhr
- Sa, 12. März 2022, 19:00 Uhr
- So, 13. März 2022, 19:00 Uhr",
                format_payday(Utc::now() + Duration::days(1))
            )
        );
    }

    #[test]
    fn test_replace_payday() {
        // event starts in 3 weeks
        let event = new_event(vec![Utc::now().naive_utc() + Duration::weeks(3)]);
        assert_eq!(
            replace_payday("${payday}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(1))
        );
        assert_eq!(
            replace_payday("${payday:7}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(2))
        );
        assert_eq!(
            replace_payday("${payday:0}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(3))
        );
        let tomorrow = (Utc::now() + Duration::days(1))
            .format_localized("%d. %B", Locale::de_DE)
            .to_string();
        assert_eq!(replace_payday("${payday:21}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:28}".into(), &event), tomorrow);

        // event starts in 3 days
        let event = new_event(vec![Utc::now().naive_utc() + Duration::days(3)]);
        assert_eq!(
            replace_payday("${payday:1}".into(), &event),
            format_payday(Utc::now() + Duration::days(2))
        );
        assert_eq!(replace_payday("${payday:2}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:3}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:14}".into(), &event), tomorrow);

        // event starts today
        let event = new_event(vec![Utc::now().naive_utc()]);
        assert_eq!(replace_payday("${payday}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:7}".into(), &event), tomorrow);

        // event started yesterday
        let event = new_event(vec![Utc::now().naive_utc() - Duration::days(1)]);
        assert_eq!(replace_payday("${payday}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:7}".into(), &event), tomorrow);
    }

    #[test]
    fn test_compare_csv_with_bookings() {
        // matching bookings with and without errors
        let csv = ";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
09.03.2022;09.03.2022;51 Überweisungsgutschr.;931;Max Mustermann;0;DE62500105176261449571;10517962;SOLADES1FDS;22-1423;;EUR;27,00;H
10.03.2022;10.03.2022;54 Überweisungsgutschr.;931;Erika Mustermann;0;DE91500105176171781279;10517962;SOLADES1FDS;Erika 22-1425 Mustermann;;EUR;33,50;H
10.03.2022;10.03.2022;78 Euro-Überweisung;931;Lieschen Müller;0;DE21500105179625862911;10517962;GENODES1VBH;Lieschen Müller 22-1456;;EUR;27,00;H
10.03.2022;10.03.2022;90 Euro-Überweisung;931;Otto Normalverbraucher;0;DE21500105179625862911;10517962;GENODES1VBH;Otto Normalverbraucher, Test-Kurs,22-1467;;EUR;45,90;H
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";
        let mut bookings = vec![
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F10".into(),
                27.0,
                "22-1423".into(),
                false,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F11".into(),
                27.0,
                "22-1425".into(),
                false,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F12".into(),
                27.0,
                "22-1456".into(),
                true,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F12".into(),
                45.90,
                "22-1467".into(),
                false,
            ),
        ];

        assert_eq!(
            compare_csv_with_bookings(csv, &mut bookings).unwrap(),
            (
                vec![
                    VerifyPaymentBookingRecord::new(
                        "Test-Kurs".into(),
                        "F10".into(),
                        27.0,
                        "22-1423".into(),
                        false,
                    ),
                    VerifyPaymentBookingRecord::new(
                        "Test-Kurs".into(),
                        "F12".into(),
                        45.90,
                        "22-1467".into(),
                        false,
                    )
                ],
                vec![
                    VerifyPaymentResult::new(
                        "2 bezahlte Buchungen".into(),
                        vec!["22-1423".into(), "22-1467".into()]
                    ),
                    VerifyPaymentResult::new(
                        "2 Buchungen mit Problemen".into(),
                        vec!["22-1425 / Betrag falsch: erwartet 27,00\u{a0}€ != überwiesen 33,50\u{a0}€".into(),
                             "22-1456 / Doppelt bezahlt: Buchung war schon als bezahlt markiert".into()]
                    ),
                    VerifyPaymentResult::new(
                        "1 nicht erkannte Buchung".into(),
                        vec!["801 / Test GmbH / DE92500105174132432988 / Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH / -24,15\u{a0}€".into()]
                    )
                ]
            )
        );

        // no matching bookings
        let csv = ";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";
        let mut bookings = vec![
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F10".into(),
                27.0,
                "22-1423".into(),
                false,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F11".into(),
                27.0,
                "22-1425".into(),
                false,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F12".into(),
                27.0,
                "22-1456".into(),
                true,
            ),
            VerifyPaymentBookingRecord::new(
                "Test-Kurs".into(),
                "F12".into(),
                45.90,
                "22-1467".into(),
                false,
            ),
        ];

        assert_eq!(
            compare_csv_with_bookings(csv, &mut bookings).unwrap(),
            (
                vec![],
                vec![
                    VerifyPaymentResult::new("0 bezahlte Buchungen".into(), vec![]),
                    VerifyPaymentResult::new("0 Buchungen mit Problemen".into(), vec![]),
                    VerifyPaymentResult::new(
                        "1 nicht erkannte Buchung".into(),
                        vec!["801 / Test GmbH / DE92500105174132432988 / Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH / -24,15\u{a0}€".into()]
                    )
                ]
            )
        );
    }

    fn format_payday(date_time: DateTime<Utc>) -> String {
        date_time
            .format_localized("%d. %B", Locale::de_DE)
            .to_string()
    }

    fn new_event(dates: Vec<NaiveDateTime>) -> Event {
        Event::new(
            String::from("id"),
            String::from("sheet_id"),
            0,
            EventType::Fitness,
            String::from("name"),
            0,
            true,
            false,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            dates,
            None,
            0,
            0,
            0,
            0.0,
            0.0,
            0,
            0,
            String::from("location"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        )
    }
}
