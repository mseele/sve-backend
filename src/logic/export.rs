use crate::{
    db,
    models::{Event, EventId, EventSubscription, ToEuro},
};
use anyhow::{Result, anyhow};
use chrono::Locale;
use printpdf::{
    Color, Line, LinePoint, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfParseErrorSeverity,
    PdfSaveOptions, PdfWarnMsg, Point, Polygon, PolygonRing, Pt, Rgb, Svg, TextItem, TextMatrix,
    XObjectTransform,
};
use simple_excel_writer::{CellValue, Column, Row, ToCellValue, Workbook};
use sqlx::PgPool;
use tracing::warn;

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

    let mut custom_fields = Vec::new();
    for custom_field in event.custom_fields.iter() {
        sheet.add_column(Column { width: 20.0 });
        custom_fields.push(custom_field.name.clone());
    }

    sheet.add_column(Column { width: 100.0 });

    workbook.write_sheet(&mut sheet, |sheet_writer| {
        let mut row = Row::new();
        row.add_cell("Id");
        row.add_cell("Buchungsdatum");
        row.add_cell("Vorname");
        row.add_cell("Nachname");
        row.add_cell("Straße");
        row.add_cell("PLZ");
        row.add_cell("Email");
        row.add_cell("Telefon");
        row.add_cell("Mitglied");
        row.add_cell("Betrag");
        row.add_cell("Buchungsnr");
        row.add_cell("Bezahlt");
        for custom_field in custom_fields.iter() {
            row.add_cell(custom_field.to_owned());
        }
        row.add_cell("Kommentar");

        sheet_writer.append_row(row)?;

        for value in subscribers {
            let mut row = Row::new();
            row.add_cell(value.id.to_string());
            row.add_cell(value.created.naive_utc());
            row.add_cell(value.first_name);
            row.add_cell(value.last_name);
            row.add_cell(value.street);
            row.add_cell(value.city);
            row.add_cell(value.email);
            row.add_cell(opt(value.phone));
            row.add_cell(bool(value.member));
            row.add_cell(event.price(value.member).to_euro());
            row.add_cell(value.payment_id);
            row.add_cell(bool(value.payed));
            for (index, _) in custom_fields.iter().enumerate() {
                row.add_cell(opt(value.custom_values.get(index).cloned()));
            }
            row.add_cell(opt(value.comment));
            sheet_writer.append_row(row)?;
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
    let mut doc = PdfDocument::new("Teilnehmerliste");
    let mut warnings = Vec::<PdfWarnMsg>::new();

    let font_regular = {
        let font = printpdf::ParsedFont::from_bytes(
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            0,
            &mut Vec::new(),
        )
        .ok_or_else(|| anyhow!("Failed to parse Inter-Regular font"))?;
        doc.add_font(&font)
    };

    let font_medium = {
        let font = printpdf::ParsedFont::from_bytes(
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
            0,
            &mut Vec::new(),
        )
        .ok_or_else(|| anyhow!("Failed to parse Inter-Medium font"))?;
        doc.add_font(&font)
    };

    for chunk in participants.chunks(16) {
        let ops = create_participant_list_page(
            &mut doc,
            &font_regular,
            &font_medium,
            event_name,
            day_and_time,
            chunk,
            dates,
        )?;
        let page = PdfPage::new(Mm(297.0), Mm(210.0), ops);
        doc.pages.push(page);
    }

    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    for w in warnings
        .iter()
        .filter(|w| w.severity != PdfParseErrorSeverity::Info)
    {
        warn!("PDF warning (participant list): {:?}", w);
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn create_participant_list_page(
    doc: &mut PdfDocument,
    font_regular: &printpdf::FontId,
    font_medium: &printpdf::FontId,
    event_name: &str,
    day_and_time: &String,
    participants: &[(String, String)],
    dates: &[String],
) -> Result<Vec<Op>> {
    let mut ops = Vec::new();
    let mut warnings = Vec::<PdfWarnMsg>::new();

    // Parse SVG logo
    let svg_xobject = Svg::parse(include_str!("../assets/logo.svg"), &mut warnings)
        .map_err(|e| anyhow!("Failed to parse SVG: {e}"))?;
    let svg_id = doc.add_xobject(&svg_xobject);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(14.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(190.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("SV Eutingen 1947 e.V.")],
    });
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(11.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(183.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(format!(
            "Teilnehmerliste \u{2022} {event_name} \u{2022} {day_and_time}"
        ))],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb::new(162.0 / 255.0, 33.0 / 255.0, 34.0 / 255.0, None)),
    });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(20.0), Mm(180.0)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(277.0), Mm(180.0)),
                    bezier: false,
                },
            ],
            ..Default::default()
        },
    });

    // SVG logo
    ops.push(Op::UseXobject {
        id: svg_id,
        transform: XObjectTransform {
            translate_x: Some(Mm(265.0).into_pt()),
            translate_y: Some(Mm(181.0).into_pt()),
            scale_x: Some(0.23),
            scale_y: Some(0.23),
            ..Default::default()
        },
    });

    // table
    let mut y = 173.0;
    let line_height = 9.0;

    // header background
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb::new(241.0 / 255.0, 243.0 / 255.0, 244.0 / 255.0, None)),
    });
    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint {
                        p: Point::new(Mm(20.0), Mm(y)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(277.0), Mm(y)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(277.0), Mm(y - line_height)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(20.0), Mm(y - line_height)),
                        bezier: false,
                    },
                ],
            }],
            ..Default::default()
        },
    });

    for l in 0..18 {
        // horizontal line
        ops.push(Op::SetOutlineColor {
            col: Color::Rgb(Rgb::new(189.0 / 255.0, 193.0 / 255.0, 198.0 / 255.0, None)),
        });
        ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point::new(Mm(20.0), Mm(y)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(277.0), Mm(y)),
                        bezier: false,
                    },
                ],
                ..Default::default()
            },
        });

        if l == 0 {
            // header row
            let y_text = y - (line_height - 3.0);

            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetFillColor {
                col: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
            });
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFont {
                font: PdfFontHandle::External(font_medium.clone()),
                size: Pt(11.0),
            });
            ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(Mm(22.0).into_pt(), Mm(y_text).into_pt()),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::from("Teilnehmer")],
            });
            ops.push(Op::EndTextSection);

            let mut x = 107.0;
            for c in 0..10 {
                // vertical line
                ops.push(Op::DrawLine {
                    line: Line {
                        points: vec![
                            LinePoint {
                                p: Point::new(Mm(x), Mm(y)),
                                bezier: false,
                            },
                            LinePoint {
                                p: Point::new(Mm(x), Mm(20.0)),
                                bezier: false,
                            },
                        ],
                        ..Default::default()
                    },
                });

                if dates.len() > c {
                    // header text
                    ops.push(Op::StartTextSection);
                    ops.push(Op::SetFillColor {
                        col: Color::Rgb(Rgb::new(
                            183.0 / 255.0,
                            183.0 / 255.0,
                            183.0 / 255.0,
                            None,
                        )),
                    });
                    ops.push(Op::SetFont {
                        font: PdfFontHandle::External(font_medium.clone()),
                        size: Pt(11.0),
                    });
                    ops.push(Op::SetTextMatrix {
                        matrix: TextMatrix::Translate(Mm(x + 3.0).into_pt(), Mm(y_text).into_pt()),
                    });
                    ops.push(Op::ShowText {
                        items: vec![TextItem::from(dates[c].as_str())],
                    });
                    ops.push(Op::EndTextSection);
                }

                x += 17.0;
            }
            ops.push(Op::RestoreGraphicsState);
        } else if participants.len() >= l {
            let (first_name, last_name) = &participants[l - 1];
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetFillColor {
                col: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
            });
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFont {
                font: PdfFontHandle::External(font_regular.clone()),
                size: Pt(11.0),
            });
            ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(
                    Mm(22.0).into_pt(),
                    Mm(y - (line_height - 3.0)).into_pt(),
                ),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::from(format!("{first_name} {last_name}"))],
            });
            ops.push(Op::EndTextSection);
            ops.push(Op::RestoreGraphicsState);
        }

        y -= line_height;
    }

    Ok(ops)
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
    let mut doc = PdfDocument::new("Teilnahmebescheinigung");
    let mut warnings = Vec::<PdfWarnMsg>::new();

    let font_regular = {
        let font = printpdf::ParsedFont::from_bytes(
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            0,
            &mut Vec::new(),
        )
        .ok_or_else(|| anyhow!("Failed to parse Inter-Regular font"))?;
        doc.add_font(&font)
    };

    let font_medium = {
        let font = printpdf::ParsedFont::from_bytes(
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
            0,
            &mut Vec::new(),
        )
        .ok_or_else(|| anyhow!("Failed to parse Inter-Medium font"))?;
        doc.add_font(&font)
    };

    let font_italic = {
        let font = printpdf::ParsedFont::from_bytes(
            include_bytes!("../assets/fonts/Inter-Italic.ttf"),
            0,
            &mut Vec::new(),
        )
        .ok_or_else(|| anyhow!("Failed to parse Inter-Italic font"))?;
        doc.add_font(&font)
    };

    let mut ops = Vec::new();

    // Parse SVG logo
    let svg_xobject = Svg::parse(include_str!("../assets/logo.svg"), &mut warnings)
        .map_err(|e| anyhow!("Failed to parse SVG: {e}"))?;
    let svg_id = doc.add_xobject(&svg_xobject);

    // Parse signature image
    let sign_image =
        printpdf::RawImage::decode_from_bytes(include_bytes!("../assets/sign.jpg"), &mut warnings)
            .map_err(|e| anyhow!("Failed to decode signature image: {e}"))?;
    let sign_image_id = doc.add_image(&sign_image);

    // header
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(18.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(252.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("SV Eutingen 1947 e.V.")],
    });
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(11.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(244.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "Fussball \u{2022} Fitness \u{2022} Ern\u{e4}hrung \u{2022} Volleyball",
        )],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb::new(162.0 / 255.0, 33.0 / 255.0, 34.0 / 255.0, None)),
    });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(20.0), Mm(240.0)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(141.0), Mm(240.0)),
                    bezier: false,
                },
            ],
            ..Default::default()
        },
    });

    // SVG logo
    ops.push(Op::UseXobject {
        id: svg_id,
        transform: XObjectTransform {
            translate_x: Some(Mm(156.0).into_pt()),
            translate_y: Some(Mm(220.0).into_pt()),
            scale_x: Some(0.75),
            scale_y: Some(0.75),
            ..Default::default()
        },
    });

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_medium.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(213.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Teilnahmebest\u{e4}tigung")],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_medium.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(192.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(format!("{first_name} {last_name}"))],
    });
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            " hat erfolgreich an folgendem Kurs teilgenommen:",
        )],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_medium.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(177.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(event_name)],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(167.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Kursbeginn:")],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(48.0).into_pt(), Mm(167.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(first_date)],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(96.0).into_pt(), Mm(167.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Kursende:")],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(119.0).into_pt(), Mm(167.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(last_date)],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(159.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Kosten:")],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(48.0).into_pt(), Mm(159.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(price)],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(151.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Termine:")],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(48.0).into_pt(), Mm(151.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(dates)],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(137.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "Der Kurs wurde von einer lizenzierten Trainerin bzw. einem lizenzierten Trainer geleitet.",
        )],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(122.0).into_pt()),
    });
    ops.push(Op::SetLineHeight { lh: Pt(18.0) });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "Wir bedanken uns sehr herzlich und freuen uns \u{fc}ber eine erneute Teilnahme an den",
        )],
    });
    ops.push(Op::AddLineBreak);
    ops.push(Op::ShowText {
        items: vec![TextItem::from("verschiedenen Sportangeboten des Vereins.")],
    });
    ops.push(Op::EndTextSection);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(81.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "F\u{fc}r den Sportverein Eutingen 1947 e.V.",
        )],
    });
    ops.push(Op::EndTextSection);

    // Signature image
    ops.push(Op::UseXobject {
        id: sign_image_id,
        transform: XObjectTransform {
            translate_x: Some(Mm(24.0).into_pt()),
            translate_y: Some(Mm(67.0).into_pt()),
            scale_x: Some(1.0),
            scale_y: Some(1.0),
            ..Default::default()
        },
    });

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_italic.clone()),
        size: Pt(12.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(20.0).into_pt(), Mm(61.0).into_pt()),
    });
    ops.push(Op::SetLineHeight { lh: Pt(28.0) });
    ops.push(Op::ShowText {
        items: vec![TextItem::from("Gez. Sebastian Lazar")],
    });
    ops.push(Op::AddLineBreak);
    ops.push(Op::ShowText {
        items: vec![TextItem::from("- 1. Vorsitzender -")],
    });
    ops.push(Op::EndTextSection);

    // footer
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::External(font_regular.clone()),
        size: Pt(10.0),
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(34.0).into_pt(), Mm(24.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "SV Eutingen 1947 e.V. \u{2022} Marktstr. 84 \u{2022} 72184 Eutingen im G\u{e4}u \u{2022} info@sv-eutingen.de",
        )],
    });
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Translate(Mm(64.0).into_pt(), Mm(19.0).into_pt()),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::from(
            "www.sv-eutingen.de \u{2022} facebook.com/sveutingen",
        )],
    });
    ops.push(Op::EndTextSection);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages.push(page);

    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    for w in warnings
        .iter()
        .filter(|w| w.severity != PdfParseErrorSeverity::Info)
    {
        warn!("PDF warning (participation confirmation): {:?}", w);
    }
    Ok(bytes)
}
