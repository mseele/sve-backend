use crate::{
    db,
    models::{Event, EventId, EventSubscription, ToEuro},
};
use anyhow::{anyhow, Result};
use chrono::Locale;
use image::codecs::jpeg::JpegDecoder;
use printpdf::{
    Color, Image, ImageTransform, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerIndex,
    PdfPageReference, Point, Rgb, Svg, SvgTransform,
};
use simple_excel_writer::{row, CellValue, Column, Row, ToCellValue, Workbook};
use sqlx::PgPool;
use std::io::Cursor;

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
    for subscriber in subscribers {
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

        for value in subscribers {
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
                event.price(value.member).to_euro(),
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

/// create a participants list for the event bookings of the given event id
pub(crate) async fn event_participants_list(
    pool: &PgPool,
    event_id: EventId,
) -> Result<(String, Vec<u8>)> {
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
    // extract the participants
    let mut participants = Vec::new();
    for subscriber in subscribers {
        if subscriber.enrolled {
            participants.push((subscriber.first_name, subscriber.last_name));
        }
    }

    let (day_and_time, dates) = if let Some(custom_date) = event.custom_date {
        (custom_date, vec![])
    } else {
        let mut custom_date = None;
        let mut dates = Vec::new();
        for date in event.dates {
            if custom_date.is_none() {
                custom_date = Some(
                    date.format_localized("%A, %H:%M Uhr", Locale::de_DE)
                        .to_string(),
                );
            }
            dates.push(date.format_localized("%d.%m.", Locale::de_DE).to_string());
        }
        (custom_date.unwrap_or_else(|| "-".into()), dates)
    };

    // create a filename and return it with the bytes
    let filename = format!("{}.pdf", event.name.replace(' ', "_").to_lowercase());

    let bytes = actix_web::rt::spawn(async move {
        create_participant_list(&event.name, &day_and_time, &participants, &dates)
    })
    .await??;

    Ok((filename, bytes))
}

fn create_participant_list(
    event_name: &str,
    day_and_time: &String,
    participants: &[(String, String)],
    dates: &[String],
) -> Result<Vec<u8>> {
    let (doc, page, layer) = PdfDocument::new("Teilnehmerliste", Mm(297.0), Mm(210.0), "Graphic");

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_ref());
    let font_regular = doc.add_external_font(&mut font_reader).unwrap();

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_ref());
    let font_medium = doc.add_external_font(&mut font_reader).unwrap();

    for (i, chunk) in participants.chunks(16).enumerate() {
        let (page, layer) = if i > 0 {
            doc.add_page(Mm(297.0), Mm(210.0), "Graphic")
        } else {
            (page, layer)
        };
        create_participant_list_page(
            doc.get_page(page),
            layer,
            &font_regular,
            &font_medium,
            event_name,
            day_and_time,
            chunk,
            dates,
        )?;
    }

    Ok(doc.save_to_bytes()?)
}

#[allow(clippy::too_many_arguments)]
fn create_participant_list_page(
    page: PdfPageReference,
    layer: PdfLayerIndex,
    font_regular: &IndirectFontRef,
    font_medium: &IndirectFontRef,
    event_name: &str,
    day_and_time: &String,
    participants: &[(String, String)],
    dates: &[String],
) -> Result<()> {
    let graphic_layer = page.get_layer(layer);
    let text_layer = page.add_layer("Text");

    // header
    text_layer.use_text(
        "SV Eutingen 1947 e.V.",
        14.0,
        Mm(20.0),
        Mm(190.0),
        font_regular,
    );
    text_layer.use_text(
        format!("Teilnehmerliste • {event_name} • {day_and_time}"),
        11.0,
        Mm(20.0),
        Mm(183.0),
        font_regular,
    );

    let line = Line {
        points: vec![
            (Point::new(Mm(20.0), Mm(180.0)), false),
            (Point::new(Mm(277.0), Mm(180.0)), false),
        ],
        has_stroke: true,
        ..Default::default()
    };
    graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
        162.0 / 255.0,
        33.0 / 255.0,
        34.0 / 255.0,
        None,
    )));
    graphic_layer.add_shape(line);

    let svg = Svg::parse(include_str!("../assets/logo.svg"))?;
    svg.add_to_layer(
        &graphic_layer,
        SvgTransform {
            translate_x: Some(Mm(265.0).into()),
            translate_y: Some(Mm(181.0).into()),
            scale_x: Some(0.23),
            scale_y: Some(0.23),
            ..Default::default()
        },
    );

    // table
    let mut y = 173.0;
    let line_height = 9.0;

    // header background
    let shape = Line {
        points: vec![
            (Point::new(Mm(20.0), Mm(y)), false),
            (Point::new(Mm(277.0), Mm(y)), false),
            (Point::new(Mm(277.0), Mm(y - line_height)), false),
            (Point::new(Mm(20.0), Mm(y - line_height)), false),
        ],
        is_closed: true,
        has_fill: true,
        ..Default::default()
    };
    graphic_layer.set_fill_color(Color::Rgb(Rgb::new(
        241.0 / 255.0,
        243.0 / 255.0,
        244.0 / 255.0,
        None,
    )));
    graphic_layer.add_shape(shape);

    for l in 0..18 {
        // horizontal line
        let line = Line {
            points: vec![
                (Point::new(Mm(20.0), Mm(y)), false),
                (Point::new(Mm(277.0), Mm(y)), false),
            ],
            has_stroke: true,
            ..Default::default()
        };
        graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
            189.0 / 255.0,
            193.0 / 255.0,
            198.0 / 255.0,
            None,
        )));
        graphic_layer.add_shape(line);

        if l == 0 {
            // header row
            let y_text = y - (line_height - 3.0);

            text_layer.use_text("Teilnehmer", 11.0, Mm(22.0), Mm(y_text), font_medium);

            let mut x = 107.0;
            for c in 0..10 {
                // vertical line
                let line = Line {
                    points: vec![
                        (Point::new(Mm(x), Mm(y)), false),
                        (Point::new(Mm(x), Mm(20.0)), false),
                    ],
                    has_stroke: true,
                    ..Default::default()
                };
                graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
                    189.0 / 255.0,
                    193.0 / 255.0,
                    198.0 / 255.0,
                    None,
                )));
                graphic_layer.add_shape(line);

                if dates.len() > c {
                    // header text
                    text_layer.begin_text_section();
                    text_layer.set_fill_color(Color::Rgb(Rgb::new(
                        183.0 / 255.0,
                        183.0 / 255.0,
                        183.0 / 255.0,
                        None,
                    )));
                    text_layer.set_font(font_medium, 11.0);
                    text_layer.set_text_cursor(Mm(x + 3.0), Mm(y_text));
                    text_layer.write_text(&dates[c], font_medium);
                    text_layer.end_text_section();
                }

                x += 17.0;
            }

            // reset text color
            text_layer.set_fill_color(Color::Rgb(Rgb::new(
                0.0 / 255.0,
                0.0 / 255.0,
                0.0 / 255.0,
                None,
            )));
        } else if participants.len() >= l {
            let (first_name, last_name) = &participants[l - 1];
            text_layer.use_text(
                format!("{first_name} {last_name}"),
                11.0,
                Mm(22.0),
                Mm(y - (line_height - 3.0)),
                font_regular,
            );
        }

        y -= line_height;
    }

    Ok(())
}
