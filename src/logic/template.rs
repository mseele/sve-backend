use anyhow::Result;
use chrono::{DateTime, Duration, Locale, Utc};
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use serde::Serialize;

use crate::models::{Event, EventBooking, ToEuro};

#[derive(Serialize)]
pub(crate) struct EventTemplateData<'a> {
    #[serde(rename = "firstname")]
    first_name: &'a str,
    #[serde(rename = "lastname")]
    last_name: &'a str,
    name: &'a str,
    location: &'a str,
    price: String,
    dates: String,
    payment_id: Option<String>,
    #[serde(rename = "link")]
    prebooking_link: Option<String>,
    direct_booking: Option<bool>,
}

impl<'a> EventTemplateData<'a> {
    pub(crate) fn new(
        booking: &'a EventBooking,
        event: &'a Event,
        payment_id: Option<String>,
        prebooking_link: Option<String>,
        direct_booking: Option<bool>,
    ) -> Self {
        Self {
            first_name: booking.first_name.trim(),
            last_name: booking.last_name.trim(),
            name: event.name.trim(),
            location: &event.location,
            price: booking.cost(event).to_euro(),
            dates: format_dates(&event),
            payment_id,
            prebooking_link,
            direct_booking,
        }
    }
}

#[derive(Clone, Copy)]
struct PaydayHelper<'a> {
    first_event_date: Option<&'a DateTime<Utc>>,
}

impl<'a> PaydayHelper<'a> {
    fn new(event: &'a Event) -> Self {
        Self {
            first_event_date: event.dates.first(),
        }
    }
}

impl HelperDef for PaydayHelper<'_> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        if let Some(first_date) = self.first_event_date {
            // default value is 14 days
            let mut days = 14;

            // overwrite with the first param - if available
            let param = h.param(0);
            if let Some(param) = param {
                days = param
                    .value()
                    .as_i64()
                    .ok_or(RenderError::new("payday extension is no integer"))?;
            }

            let mut payday = *first_date - Duration::days(days.into());
            let tomorrow = Utc::now() + Duration::days(1);
            if payday < tomorrow {
                payday = tomorrow
            }

            out.write(&payday.format_localized("%d. %B", Locale::de_DE).to_string())?;
        }

        Ok(())
    }
}

fn format_dates(event: &Event) -> String {
    event
        .dates
        .iter()
        .map(|d| {
            d.format_localized("- %a., %d. %B %Y, %H:%M Uhr", Locale::de_DE)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn render_booking<'a>(
    template: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    payment_id: Option<String>,
    prebooking_link: Option<String>,
    direct_booking: Option<bool>,
) -> Result<String> {
    Ok(render(
        template,
        EventTemplateData::new(booking, event, payment_id, prebooking_link, direct_booking),
        PaydayHelper::new(event),
    )?)
}

fn render(template: &str, data: EventTemplateData, payday_helper: PaydayHelper) -> Result<String> {
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.register_helper("payday", Box::new(payday_helper));

    let result = handlebars.render_template(template, &data)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EventType, LifecycleStatus};
    use bigdecimal::{BigDecimal, FromPrimitive};
    use chrono::{DateTime, Duration, Locale, TimeZone, Utc};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_render_booking() {
        let booking_member = EventBooking::new(
            0,
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
            1,
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
            0,
            Utc::now(),
            None,
            EventType::Fitness,
            LifecycleStatus::Draft,
            String::from("FitForFun"),
            0,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            vec![
                Utc.ymd(2022, 3, 7).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 8).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 9).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 10).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 11).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 12).and_hms(19, 00, 00),
                Utc.ymd(2022, 3, 13).and_hms(19, 00, 00),
            ],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(5).unwrap(),
            BigDecimal::from_i8(10).unwrap(),
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        );

        assert_eq!(
            render_booking(
                "{{firstname}} {{lastname}} {{name}} {{location}} {{price}} {{payday 0}} {{payment_id}}
{{dates}}",
                &booking_member,
                &event,
                None,
                None,
                None
            )
            .unwrap(),
            format!(
                "Max Mustermann FitForFun Turn- & Festhalle Eutingen 5,00 € {} 
- Mo., 07. März 2022, 19:00 Uhr
- Di., 08. März 2022, 19:00 Uhr
- Mi., 09. März 2022, 19:00 Uhr
- Do., 10. März 2022, 19:00 Uhr
- Fr., 11. März 2022, 19:00 Uhr
- Sa., 12. März 2022, 19:00 Uhr
- So., 13. März 2022, 19:00 Uhr",
                format_payday(Utc::now() + Duration::days(1))
            ),
        );
        assert_eq!(
            render_booking(
                "{{firstname}} {{lastname}} {{name}} {{location}} {{price}} {{payday 0}} {{payment_id}}
{{dates}}",
                &booking_non_member,
                &event,
                Some(String::from("22-1012")),
                None,
                None
            )
            .unwrap(),
            format!(
                "Max Mustermann FitForFun Turn- & Festhalle Eutingen 10,00 € {} 22-1012
- Mo., 07. März 2022, 19:00 Uhr
- Di., 08. März 2022, 19:00 Uhr
- Mi., 09. März 2022, 19:00 Uhr
- Do., 10. März 2022, 19:00 Uhr
- Fr., 11. März 2022, 19:00 Uhr
- Sa., 12. März 2022, 19:00 Uhr
- So., 13. März 2022, 19:00 Uhr",
                format_payday(Utc::now() + Duration::days(1))
            )
        );

        assert_eq!(
            render_booking(
                "{{link}}",
                &booking_member,
                &event,
                None,
                Some("booking_link".into()),
                None
            )
            .unwrap(),
            "booking_link"
        );

        let template = "{{#if direct_booking}}
Platz direkt gebucht.
{{else}}
Platz als Wartelistennachrücker gebucht.{{/if}}";
        assert_eq!(
            render_booking(template, &booking_member, &event, None, None, Some(true)).unwrap(),
            "Platz direkt gebucht.
",
        );
        assert_eq!(
            render_booking(template, &booking_member, &event, None, None, Some(false)).unwrap(),
            "Platz als Wartelistennachrücker gebucht.",
        );
        assert_eq!(
            render_booking(template, &booking_member, &event, None, None, None).unwrap(),
            "Platz als Wartelistennachrücker gebucht.",
        );

        // event starts in 3 weeks
        let event = new_event(vec![Utc::now() + Duration::weeks(3)]);
        assert_eq!(
            render_booking("{{payday}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::weeks(1))
        );
        assert_eq!(
            render_booking("{{payday 7}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::weeks(2))
        );
        assert_eq!(
            render_booking("{{payday 0}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::weeks(3))
        );
        let tomorrow = (Utc::now() + Duration::days(1))
            .format_localized("%d. %B", Locale::de_DE)
            .to_string();
        assert_eq!(
            render_booking("{{payday 21}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 28}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );

        // event starts in 3 days
        let event = new_event(vec![Utc::now() + Duration::days(3)]);
        assert_eq!(
            render_booking("{{payday 1}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::days(2))
        );
        assert_eq!(
            render_booking("{{payday 2}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 3}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 14}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );

        // event starts today
        let event = new_event(vec![Utc::now()]);
        assert_eq!(
            render_booking("{{payday}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 7}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );

        // event started yesterday
        let event = new_event(vec![Utc::now() - Duration::days(1)]);
        assert_eq!(
            render_booking("{{payday}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 7}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
    }

    fn format_payday(date_time: DateTime<Utc>) -> String {
        date_time
            .format_localized("%d. %B", Locale::de_DE)
            .to_string()
    }

    fn new_event(dates: Vec<DateTime<Utc>>) -> Event {
        Event::new(
            0,
            Utc::now(),
            None,
            EventType::Fitness,
            LifecycleStatus::Draft,
            String::from("name"),
            0,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            dates,
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(0).unwrap(),
            BigDecimal::from_i8(0).unwrap(),
            String::from("location"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        )
    }
}
