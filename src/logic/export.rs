use crate::{
    db,
    models::{Event, EventId, EventSubscription, ToEuro},
};
use anyhow::{anyhow, Result};
use chrono::Locale;
use image::codecs::jpeg::JpegDecoder;
use printpdf::{
    Color, Image, ImageTransform, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerIndex,
    PdfPageReference, Point, Polygon, Rgb, Svg, SvgTransform,
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

    tokio::task::spawn_blocking(|| export(event, bookings, waiting_list)).await?
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
    let filename = format!("{}.xlsx", event.name.to_lowercase());

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
    let filename = format!("{}.pdf", event.name.to_lowercase());

    let bytes = tokio::task::spawn_blocking(move || {
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
        ..Default::default()
    };
    graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
        162.0 / 255.0,
        33.0 / 255.0,
        34.0 / 255.0,
        None,
    )));
    graphic_layer.add_line(line);

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
    let polygon = Polygon {
        rings: vec![vec![
            (Point::new(Mm(20.0), Mm(y)), false),
            (Point::new(Mm(277.0), Mm(y)), false),
            (Point::new(Mm(277.0), Mm(y - line_height)), false),
            (Point::new(Mm(20.0), Mm(y - line_height)), false),
        ]],
        ..Default::default()
    };
    graphic_layer.set_fill_color(Color::Rgb(Rgb::new(
        241.0 / 255.0,
        243.0 / 255.0,
        244.0 / 255.0,
        None,
    )));
    graphic_layer.add_polygon(polygon);

    for l in 0..18 {
        // horizontal line
        let line = Line {
            points: vec![
                (Point::new(Mm(20.0), Mm(y)), false),
                (Point::new(Mm(277.0), Mm(y)), false),
            ],
            ..Default::default()
        };
        graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
            189.0 / 255.0,
            193.0 / 255.0,
            198.0 / 255.0,
            None,
        )));
        graphic_layer.add_line(line);

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
                    ..Default::default()
                };
                graphic_layer.set_outline_color(Color::Rgb(Rgb::new(
                    189.0 / 255.0,
                    193.0 / 255.0,
                    198.0 / 255.0,
                    None,
                )));
                graphic_layer.add_line(line);

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

pub(crate) async fn create_participation_confirmation(
    first_name: String,
    last_name: String,
    event_name: String,
    first_date: String,
    last_date: String,
    price: String,
    dates: String,
) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(|| {
        _create_participation_confirmation(
            first_name, last_name, event_name, first_date, last_date, price, dates,
        )
    })
    .await?
}

fn _create_participation_confirmation(
    first_name: String,
    last_name: String,
    event_name: String,
    first_date: String,
    last_date: String,
    price: String,
    dates: String,
) -> Result<Vec<u8>> {
    let (doc, page, layer) =
        PdfDocument::new("Teilnahmebescheinigung", Mm(210.0), Mm(297.0), "Layer");
    let current_layer = doc.get_page(page).get_layer(layer);

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_ref());
    let font_regular = doc.add_external_font(&mut font_reader).unwrap();

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_ref());
    let font_medium = doc.add_external_font(&mut font_reader).unwrap();

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/Inter-Italic.ttf").as_ref());
    let font_italic = doc.add_external_font(&mut font_reader).unwrap();

    // header
    current_layer.use_text(
        "SV Eutingen 1947 e.V.",
        18.0,
        Mm(20.0),
        Mm(252.0),
        &font_regular,
    );
    current_layer.use_text(
        "Fussball • Fitness • Ernährung • Volleyball",
        11.0,
        Mm(20.0),
        Mm(244.0),
        &font_regular,
    );

    let line = Line {
        points: vec![
            (Point::new(Mm(20.0), Mm(240.0)), false),
            (Point::new(Mm(141.0), Mm(240.0)), false),
        ],
        ..Default::default()
    };
    let color = Color::Rgb(Rgb::new(162.0 / 255.0, 33.0 / 255.0, 34.0 / 255.0, None));
    current_layer.set_outline_color(color);
    current_layer.add_line(line);

    let svg = Svg::parse(include_str!("../assets/logo.svg"))?;
    svg.add_to_layer(
        &current_layer,
        SvgTransform {
            translate_x: Some(Mm(156.0).into()),
            translate_y: Some(Mm(220.0).into()),
            scale_x: Some(0.75),
            scale_y: Some(0.75),
            ..Default::default()
        },
    );

    current_layer.use_text(
        "Teilnahmebestätigung",
        12.0,
        Mm(20.0),
        Mm(213.0),
        &font_medium,
    );

    current_layer.begin_text_section();
    current_layer.set_font(&font_medium, 12.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(192.0));
    current_layer.write_text(format!("{first_name} {last_name}"), &font_medium);
    current_layer.set_font(&font_regular, 12.0);
    current_layer.write_text(
        " hat erfolgreich an folgendem Kurs teilgenommen:",
        &font_regular,
    );
    current_layer.end_text_section();

    current_layer.use_text(event_name, 12.0, Mm(20.0), Mm(177.0), &font_medium);

    current_layer.use_text("Kursbeginn:", 12.0, Mm(20.0), Mm(167.0), &font_regular);
    current_layer.use_text(first_date, 12.0, Mm(48.0), Mm(167.0), &font_regular);
    current_layer.use_text("Kursende:", 12.0, Mm(96.0), Mm(167.0), &font_regular);
    current_layer.use_text(last_date, 12.0, Mm(119.0), Mm(167.0), &font_regular);
    current_layer.use_text("Kosten:", 12.0, Mm(20.0), Mm(159.0), &font_regular);
    current_layer.use_text(price, 12.0, Mm(48.0), Mm(159.0), &font_regular);
    current_layer.use_text("Termine:", 12.0, Mm(20.0), Mm(151.0), &font_regular);
    current_layer.use_text(dates, 12.0, Mm(48.0), Mm(151.0), &font_regular);

    current_layer.use_text(
        "Der Kurs wurde von einer lizenzierten Trainerin bzw. einem lizenzierten Trainer geleitet.",
        12.0,
        Mm(20.0),
        Mm(137.0),
        &font_regular,
    );

    current_layer.begin_text_section();
    current_layer.set_font(&font_regular, 12.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(122.0));
    current_layer.set_line_height(18.0);
    current_layer.write_text(
        "Wir bedanken uns sehr herzlich und freuen uns über eine erneute Teilnahme an den",
        &font_regular,
    );
    current_layer.add_line_break();
    current_layer.write_text("verschiedenen Sportangeboten des Vereins.", &font_regular);
    current_layer.end_text_section();

    current_layer.use_text(
        "Für den Sportverein Eutingen 1947 e.V.",
        12.0,
        Mm(20.0),
        Mm(81.0),
        &font_regular,
    );

    let mut reader = Cursor::new(include_bytes!("../assets/sign.jpg").as_ref());
    let image = Image::try_from(JpegDecoder::new(&mut reader)?)?;
    image.add_to_layer(
        current_layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(24.0)),
            translate_y: Some(Mm(67.0)),
            scale_x: Some(1.0),
            scale_y: Some(1.0),
            ..Default::default()
        },
    );

    current_layer.begin_text_section();
    current_layer.set_font(&font_italic, 12.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(61.0));
    current_layer.set_line_height(28.0);
    current_layer.write_text("Gez. Sebastian Lazar", &font_italic);
    current_layer.add_line_break();
    current_layer.write_text("- 1. Vorsitzender -", &font_italic);
    current_layer.end_text_section();

    // footer
    current_layer.use_text(
        "SV Eutingen 1947 e.V. • Marktstr. 84 • 72184 Eutingen im Gäu • info@sv-eutingen.de",
        10.0,
        Mm(34.0),
        Mm(24.0),
        &font_regular,
    );
    current_layer.use_text(
        "www.sv-eutingen.de • facebook.com/sveutingen",
        10.0,
        Mm(64.0),
        Mm(19.0),
        &font_regular,
    );

    Ok(doc.save_to_bytes()?)
}
