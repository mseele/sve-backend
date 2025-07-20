use super::events;
use crate::models::{
    Event, EventBooking, EventSubscription, MembershipApplication, ToEuro, UnpaidEventBooking,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Locale, Utc};
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderErrorReason,
};
use serde::Serialize;

#[derive(Serialize)]
struct BookingTemplateData<'a> {
    firstname: &'a str,
    lastname: &'a str,
    name: &'a str,
    location: &'a str,
    price: String,
    dates: String,
    payment_details: Option<String>,
    payment_id: Option<String>,
    link: Option<String>,
    direct_booking: Option<bool>,
}

impl<'a> BookingTemplateData<'a> {
    fn from_booking(
        booking: &'a EventBooking,
        event: &'a Event,
        payment_id: Option<String>,
        prebooking_link: Option<String>,
        direct_booking: Option<bool>,
    ) -> Self {
        Self {
            firstname: booking.first_name.trim(),
            lastname: booking.last_name.trim(),
            name: event.name.trim(),
            location: event.location.trim(),
            price: booking.price(event).to_euro(),
            dates: format_dates(&event.dates),
            payment_details: format_payment_details(event, &payment_id),
            payment_id,
            link: prebooking_link,
            direct_booking,
        }
    }

    fn from_unpaid_booking(booking: &'a UnpaidEventBooking, event: &'a Event) -> Self {
        let payment_id = Some(booking.payment_id.clone());
        Self {
            firstname: booking.first_name.trim(),
            lastname: booking.last_name.trim(),
            name: event.name.trim(),
            location: event.location.trim(),
            price: booking.price.to_euro(),
            dates: format_dates(&event.dates),
            payment_details: format_payment_details(event, &payment_id),
            payment_id,
            link: None,
            direct_booking: None,
        }
    }
}

#[derive(Serialize)]
struct ScheduleChangeTemplateData<'a> {
    firstname: &'a str,
    name: &'a str,
    removed_dates: String,
    new_dates: String,
}

impl<'a> ScheduleChangeTemplateData<'a> {
    fn new(booking: &'a EventBooking, event: &'a Event, removed_dates: &[DateTime<Utc>]) -> Self {
        let now = Utc::now();
        Self {
            firstname: booking.first_name.trim(),
            name: event.name.trim(),
            removed_dates: format_dates(removed_dates),
            new_dates: format_and_filter_dates(&event.dates, |d| d > &&now),
        }
    }
}

#[derive(Serialize)]
struct ReminderTemplateData<'a> {
    firstname: &'a str,
    name: &'a str,
    location: &'a str,
    start_date: String,
    start_time: String,
}

impl<'a> ReminderTemplateData<'a> {
    fn new(event: &'a Event, subscription: &'a EventSubscription) -> Result<Self> {
        let first_date = event
            .dates
            .first()
            .ok_or_else(|| anyhow!("Attribute 'sort_index' is missing"))?;

        let start_date = first_date
            .format_localized("%A, %-d. %B %Y", Locale::de_DE)
            .to_string();

        let start_time = first_date
            .format_localized("%H:%M Uhr", Locale::de_DE)
            .to_string();

        Ok(Self {
            firstname: subscription.first_name.trim(),
            name: event.name.trim(),
            location: event.location.trim(),
            start_date,
            start_time,
        })
    }
}

#[derive(Serialize)]
struct ParticipationConfirmationData<'a> {
    firstname: &'a str,
    name: &'a str,
}

impl<'a> ParticipationConfirmationData<'a> {
    fn new(event: &'a Event, subscription: &'a EventSubscription) -> Result<Self> {
        Ok(Self {
            firstname: subscription.first_name.trim(),
            name: event.name.trim(),
        })
    }
}

#[derive(Serialize)]
struct MembershipApplicationTemplateData<'a> {
    firstname: &'a str,
    newsletter: &'a bool,
}

impl<'a> MembershipApplicationTemplateData<'a> {
    fn new(membership_application: &'a MembershipApplication) -> Self {
        Self {
            firstname: membership_application.first_name.trim(),
            newsletter: &membership_application.newsletter,
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
            let custom_day = match h.param(0) {
                Some(param) => Some(param.value().as_i64().ok_or_else(|| {
                    RenderErrorReason::Other("payday extension is no integer".into())
                })?),
                None => None,
            };

            let payday = events::calculate_payday(&Utc::now(), first_date, custom_day)
                .expect("Payday calculation failed.");

            out.write(&payday.format_localized("%d. %B", Locale::de_DE).to_string())?;
        }

        Ok(())
    }
}

fn format_dates(dates: &[DateTime<Utc>]) -> String {
    format_and_filter_dates(dates, |_d| true)
}

fn format_and_filter_dates<P>(dates: &[DateTime<Utc>], predicate: P) -> String
where
    P: FnMut(&&DateTime<Utc>) -> bool,
{
    dates
        .iter()
        .filter(predicate)
        .map(|d| {
            d.format_localized("- %a., %d. %B %Y, %H:%M Uhr", Locale::de_DE)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_payment_details(event: &Event, payment_id: &Option<String>) -> Option<String> {
    match (&event.payment_account, payment_id) {
        (Some(payment_account), Some(payment_id)) => Some(format!(
            r#"{}
Verwendungszweck: {}"#,
            payment_account, payment_id,
        )),
        _ => None,
    }
}

pub(crate) fn render_booking<'a>(
    template: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    payment_id: Option<String>,
    prebooking_link: Option<String>,
    direct_booking: Option<bool>,
) -> Result<String> {
    render(
        template,
        BookingTemplateData::from_booking(
            booking,
            event,
            payment_id,
            prebooking_link,
            direct_booking,
        ),
        Some(PaydayHelper::new(event)),
    )
}

pub(crate) fn render_event_reminder<'a>(
    template: &str,
    event: &'a Event,
    subscription: &'a EventSubscription,
) -> Result<String> {
    render(
        template,
        ReminderTemplateData::new(event, subscription)?,
        None,
    )
}

pub(crate) fn render_participation_confirmation<'a>(
    template: &str,
    event: &'a Event,
    subscription: &'a EventSubscription,
) -> Result<String> {
    render(
        template,
        ParticipationConfirmationData::new(event, subscription)?,
        None,
    )
}

pub(crate) fn render_payment_reminder<'a>(
    template: &str,
    event: &'a Event,
    booking: &'a UnpaidEventBooking,
) -> Result<String> {
    render(
        template,
        BookingTemplateData::from_unpaid_booking(booking, event),
        None,
    )
}

pub(crate) fn render_schedule_change<'a>(
    template: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    removed_dates: &[DateTime<Utc>],
) -> Result<String> {
    render(
        template,
        ScheduleChangeTemplateData::new(booking, event, removed_dates),
        None,
    )
}

pub(crate) fn render_membership_application(
    template: &str,
    membership_application: &MembershipApplication,
) -> Result<String> {
    render(
        template,
        MembershipApplicationTemplateData::new(membership_application),
        None,
    )
}

fn render<D>(template: &str, data: D, payday_helper: Option<PaydayHelper>) -> Result<String>
where
    D: Serialize,
{
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_escape_fn(handlebars::no_escape);
    if let Some(payday_helper) = payday_helper {
        handlebars.register_helper("payday", Box::new(payday_helper));
    }

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
            Vec::new(),
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
            Vec::new(),
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
                Utc.with_ymd_and_hms(2022, 3, 7, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 8, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 9, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 10, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 11, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 12, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 13, 19, 00, 00).unwrap(),
            ],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(5).unwrap(),
            BigDecimal::from_i8(10).unwrap(),
            None,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            Some(String::from(
                "Sportverein Eutingen im Gäu e.V.
IBAN: DE16 6429 1010 0034 4696 05",
            )),
            None,
            None,
            false,
            Vec::new(),
        );

        assert_eq!(
            render_booking(
                "{{firstname}} {{lastname}} {{name}} {{location}} {{price}} {{payday 0}} {{payment_id}}
{{payment_details}}
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
                format_payday(Utc::now() + Duration::try_days(1).unwrap())
            ),
        );
        assert_eq!(
            render_booking(
                "{{firstname}} {{lastname}} {{name}} {{location}} {{price}} {{payday 0}} {{payment_id}}

{{payment_details}}

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

Sportverein Eutingen im Gäu e.V.
IBAN: DE16 6429 1010 0034 4696 05
Verwendungszweck: 22-1012

- Mo., 07. März 2022, 19:00 Uhr
- Di., 08. März 2022, 19:00 Uhr
- Mi., 09. März 2022, 19:00 Uhr
- Do., 10. März 2022, 19:00 Uhr
- Fr., 11. März 2022, 19:00 Uhr
- Sa., 12. März 2022, 19:00 Uhr
- So., 13. März 2022, 19:00 Uhr",
                format_payday(Utc::now() + Duration::try_days(1).unwrap())
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
        let event = new_event(vec![Utc::now() + Duration::try_weeks(3).unwrap()]);
        assert_eq!(
            render_booking("{{payday}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::try_weeks(1).unwrap())
        );
        assert_eq!(
            render_booking("{{payday 7}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::try_weeks(2).unwrap())
        );
        assert_eq!(
            render_booking("{{payday 0}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::try_weeks(3).unwrap())
        );
        let tomorrow = (Utc::now() + Duration::try_days(1).unwrap())
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
        let event = new_event(vec![Utc::now() + Duration::try_days(3).unwrap()]);
        assert_eq!(
            render_booking("{{payday 1}}", &booking_member, &event, None, None, None).unwrap(),
            format_payday(Utc::now() + Duration::try_days(2).unwrap())
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
        let event = new_event(vec![Utc::now() - Duration::try_days(1).unwrap()]);
        assert_eq!(
            render_booking("{{payday}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            render_booking("{{payday 7}}", &booking_member, &event, None, None, None).unwrap(),
            tomorrow
        );
    }

    #[test]
    fn test_render_reminder() {
        let mut event = Event::new(
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
                Utc.with_ymd_and_hms(2022, 3, 7, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 8, 19, 00, 00).unwrap(),
            ],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(5).unwrap(),
            BigDecimal::from_i8(10).unwrap(),
            None,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            None,
            None,
            None,
            false,
            Vec::new(),
        );
        let event_subscription = EventSubscription::new(
            0,
            Utc::now(),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("Haupstraße 1"),
            String::from("72184 Eutingen"),
            String::from("max@musterman.de"),
            None,
            true,
            true,
            String::from("123"),
            true,
            None,
            Vec::new(),
        );

        assert_eq!(
            render_event_reminder(
                "{{firstname}} {{name}} {{location}} {{start_date}} {{start_time}}",
                &event,
                &event_subscription,
            )
            .unwrap(),
            "Max FitForFun Turn- & Festhalle Eutingen Montag, 7. März 2022 19:00 Uhr",
        );

        event.dates = vec![
            Utc.with_ymd_and_hms(2022, 3, 23, 19, 00, 00).unwrap(),
            Utc.with_ymd_and_hms(2022, 3, 24, 19, 00, 00).unwrap(),
        ];

        assert_eq!(
            render_event_reminder(
                "{{firstname}} {{name}} {{location}} {{start_date}} {{start_time}}",
                &event,
                &event_subscription,
            )
            .unwrap(),
            "Max FitForFun Turn- & Festhalle Eutingen Mittwoch, 23. März 2022 19:00 Uhr",
        );
    }

    #[test]
    fn test_render_participation_confirmation() {
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
                Utc.with_ymd_and_hms(2022, 3, 7, 19, 00, 00).unwrap(),
                Utc.with_ymd_and_hms(2022, 3, 8, 19, 00, 00).unwrap(),
            ],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(5).unwrap(),
            BigDecimal::from_i8(10).unwrap(),
            None,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            None,
            None,
            None,
            false,
            Vec::new(),
        );
        let event_subscription = EventSubscription::new(
            0,
            Utc::now(),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("Haupstraße 1"),
            String::from("72184 Eutingen"),
            String::from("max@musterman.de"),
            None,
            true,
            true,
            String::from("123"),
            true,
            None,
            Vec::new(),
        );

        assert_eq!(
            render_participation_confirmation(
                "{{firstname}} {{name}}",
                &event,
                &event_subscription,
            )
            .unwrap(),
            "Max FitForFun",
        );
    }

    #[test]
    fn test_render_schedule_change() {
        let date_1 = Utc.with_ymd_and_hms(2022, 3, 7, 19, 00, 00).unwrap();
        let date_2 = Utc.with_ymd_and_hms(2022, 3, 8, 19, 00, 00).unwrap();
        let date_3 = Utc.with_ymd_and_hms(2100, 3, 9, 19, 00, 00).unwrap();
        let date_4 = Utc.with_ymd_and_hms(2100, 3, 10, 19, 00, 00).unwrap();
        let date_5 = Utc.with_ymd_and_hms(2100, 3, 11, 19, 00, 00).unwrap();
        let date_6 = Utc.with_ymd_and_hms(2100, 3, 12, 19, 00, 00).unwrap();

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
            vec![date_1, date_2, date_3, date_5, date_6],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(5).unwrap(),
            BigDecimal::from_i8(10).unwrap(),
            None,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            None,
            None,
            None,
            false,
            Vec::new(),
        );
        let booking = EventBooking::new(
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
            Vec::new(),
        );

        assert_eq!(
            render_schedule_change(
                r#"{{firstname}} / {{name}}
<-->
{{removed_dates}}
<-->
{{new_dates}}"#,
                &booking,
                &event,
                &[date_4],
            )
            .unwrap(),
            r#"Max / FitForFun
<-->
- Mi., 10. März 2100, 19:00 Uhr
<-->
- Di., 09. März 2100, 19:00 Uhr
- Do., 11. März 2100, 19:00 Uhr
- Fr., 12. März 2100, 19:00 Uhr"#,
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
            None,
            String::from("location"),
            String::from("booking_template"),
            None,
            None,
            None,
            false,
            Vec::new(),
        )
    }
}
