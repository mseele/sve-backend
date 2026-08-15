use super::events;
use crate::models::{
    Event, EventBooking, EventSubscription, MembershipApplication, PaymentMethod, ToEuro,
    UnpaidEventBooking,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Locale, Utc};
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderErrorReason,
    Renderable,
};
use lazy_static::lazy_static;
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
    payment_type: String,
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
            price: booking.total_price(event).to_euro(),
            dates: format_dates(&event.dates),
            payment_details: format_payment_details(
                &event.payment_account,
                &payment_id,
                &event.payment_method,
            ),
            payment_id,
            link: prebooking_link,
            direct_booking,
            payment_type: format!("{:?}", event.payment_method),
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
            payment_details: format_payment_details(
                &event.payment_account,
                &payment_id,
                &event.payment_method,
            ),
            payment_id,
            link: None,
            direct_booking: None,
            payment_type: format!("{:?}", event.payment_method),
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

#[derive(Serialize)]
struct ContactConfirmationTemplateData<'a> {
    name: &'a str,
}

impl<'a> ContactConfirmationTemplateData<'a> {
    fn new(name: &'a str) -> Self {
        Self { name: name.trim() }
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

struct EqHelper;

impl HelperDef for EqHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let param1 = h
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex("eq", 0))?;
        let param2 = h
            .param(1)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex("eq", 1))?;

        let render = param1.value() == param2.value();
        let tmpl = if render { h.template() } else { h.inverse() };
        match tmpl {
            Some(t) => t.render(r, ctx, rc, out),
            None => Ok(()),
        }
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

fn format_payment_details(
    payment_account: &Option<String>,
    payment_id: &Option<String>,
    payment_method: &PaymentMethod,
) -> Option<String> {
    if *payment_method == PaymentMethod::SepaDirectDebit {
        return None;
    }
    let account = payment_account.as_ref()?;
    let id = payment_id.as_ref()?;
    Some(format!("{}\nVerwendungszweck: {}", account, id))
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

#[derive(Serialize)]
struct GenericMailTemplateData {
    body: String,
    ps: Option<String>,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn boldize(s: &str) -> String {
    let parts: Vec<&str> = s.split("**").collect();
    let mut out = String::with_capacity(s.len());
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 1 && !part.is_empty() {
            out.push_str("<strong>");
            out.push_str(part);
            out.push_str("</strong>");
        } else {
            out.push_str(part);
        }
    }
    out
}

pub(crate) fn render_generic_mail_html(body: &str, ps: Option<&str>) -> Result<String> {
    let body = boldize(&escape_html(body)).replace('\n', "<br>");
    let ps = ps.map(|ps| escape_html(ps).replace('\n', "<br>"));

    render_html("generic_mail", &GenericMailTemplateData { body, ps })
}

pub(crate) fn render_booking_generic<'a>(
    template: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    payment_id: Option<String>,
    prebooking_link: Option<String>,
    direct_booking: Option<bool>,
    ps: Option<&str>,
) -> Result<(String, String)> {
    let text = render_booking(
        template,
        booking,
        event,
        payment_id,
        prebooking_link,
        direct_booking,
    )?;
    let html = render_generic_mail_html(&text, ps)?;

    Ok((text, html))
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

pub(crate) fn render_event_reminder_html<'a>(
    template_name: &str,
    event: &'a Event,
    subscription: &'a EventSubscription,
) -> Result<String> {
    render_html(
        template_name,
        &ReminderTemplateData::new(event, subscription)?,
    )
}

pub(crate) fn render_booking_html<'a>(
    template_name: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    payment_id: Option<String>,
    prebooking_link: Option<String>,
    direct_booking: Option<bool>,
    ps: Option<&str>,
) -> Result<String> {
    let mut html = render_html(
        template_name,
        &BookingTemplateData::from_booking(
            booking,
            event,
            payment_id,
            prebooking_link,
            direct_booking,
        ),
    )?;

    if let Some(ps) = ps {
        let ps = escape_html(ps).replace('\n', "<br>");
        html = html.replace("</body>", &format!("<p>{ps}</p></body>"));
    }

    Ok(html)
}

pub(crate) fn render_schedule_change_html<'a>(
    template_name: &str,
    booking: &'a EventBooking,
    event: &'a Event,
    removed_dates: &[DateTime<Utc>],
) -> Result<String> {
    render_html(
        template_name,
        &ScheduleChangeTemplateData::new(booking, event, removed_dates),
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

pub(crate) fn render_participation_confirmation_html<'a>(
    template_name: &str,
    event: &'a Event,
    subscription: &'a EventSubscription,
) -> Result<String> {
    render_html(
        template_name,
        &ParticipationConfirmationData::new(event, subscription)?,
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

pub(crate) fn render_payment_reminder_html<'a>(
    template_name: &str,
    event: &'a Event,
    booking: &'a UnpaidEventBooking,
) -> Result<String> {
    render_html(
        template_name,
        &BookingTemplateData::from_unpaid_booking(booking, event),
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

pub(crate) fn render_membership_application_html(
    membership_application: &MembershipApplication,
) -> Result<String> {
    render_html(
        "membership_application",
        &MembershipApplicationTemplateData::new(membership_application),
    )
}

pub(crate) fn render_contact_confirmation(template: &str, name: &str) -> Result<String> {
    render(template, ContactConfirmationTemplateData::new(name), None)
}

pub(crate) fn render_contact_confirmation_html(name: &str) -> Result<String> {
    render_html(
        "contact_confirmation",
        &ContactConfirmationTemplateData::new(name),
    )
}

fn render<D>(template: &str, data: D, payday_helper: Option<PaydayHelper>) -> Result<String>
where
    D: Serialize,
{
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.register_helper("eq", Box::new(EqHelper));
    if let Some(payday_helper) = payday_helper {
        handlebars.register_helper("payday", Box::new(payday_helper));
    }

    let result = handlebars.render_template(template, &data)?;
    Ok(result)
}

#[allow(dead_code)]
struct StaticPaydayHelper;

impl HelperDef for StaticPaydayHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        _out: &mut dyn Output,
    ) -> HelperResult {
        Ok(())
    }
}

lazy_static! {
    pub(crate) static ref HANDLEBARS: Handlebars<'static> = {
        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_escape_fn(handlebars::no_escape);

        hb.register_helper("eq", Box::new(EqHelper));
        hb.register_helper("payday", Box::new(StaticPaydayHelper));

        hb.register_partial(
            "_branding_header",
            include_str!("../../templates/_branding_header.hbs"),
        )
        .unwrap();
        hb.register_partial(
            "_branding_footer",
            include_str!("../../templates/_branding_footer.hbs"),
        )
        .unwrap();
        hb.register_partial("_head_meta", include_str!("../../templates/_head_meta.hbs"))
            .unwrap();

        hb.register_template_string(
            "cancel_booking_events",
            include_str!("../../templates/compiled/cancel_booking_events.html"),
        )
        .unwrap();
        hb.register_template_string(
            "cancel_booking_fitness",
            include_str!("../../templates/compiled/cancel_booking_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "contact_confirmation",
            include_str!("../../templates/compiled/contact_confirmation.html"),
        )
        .unwrap();
        hb.register_template_string(
            "event_reminder_events",
            include_str!("../../templates/compiled/event_reminder_events.html"),
        )
        .unwrap();
        hb.register_template_string(
            "event_reminder_fitness",
            include_str!("../../templates/compiled/event_reminder_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "membership_application",
            include_str!("../../templates/compiled/membership_application.html"),
        )
        .unwrap();
        hb.register_template_string(
            "participation_confirmation_fitness",
            include_str!("../../templates/compiled/participation_confirmation_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "payment_reminder_events",
            include_str!("../../templates/compiled/payment_reminder_events.html"),
        )
        .unwrap();
        hb.register_template_string(
            "payment_reminder_fitness",
            include_str!("../../templates/compiled/payment_reminder_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "schedule_change_events",
            include_str!("../../templates/compiled/schedule_change_events.html"),
        )
        .unwrap();
        hb.register_template_string(
            "schedule_change_fitness",
            include_str!("../../templates/compiled/schedule_change_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "waiting_list_events",
            include_str!("../../templates/compiled/waiting_list_events.html"),
        )
        .unwrap();
        hb.register_template_string(
            "waiting_list_fitness",
            include_str!("../../templates/compiled/waiting_list_fitness.html"),
        )
        .unwrap();
        hb.register_template_string(
            "generic_mail",
            include_str!("../../templates/compiled/generic_mail.html"),
        )
        .unwrap();

        hb
    };
}

pub(crate) fn render_html(name: &str, data: &impl Serialize) -> Result<String> {
    Ok(HANDLEBARS.render(name, data)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EventType, LifecycleStatus, PaymentMethod};
    use bigdecimal::{BigDecimal, FromPrimitive};
    use chrono::{DateTime, Duration, Locale, TimeZone, Utc};
    use pretty_assertions::assert_eq;

    const DB_FITNESS_TEMPLATE: &str = r#"Hallo {{firstname}},

vielen Dank für Dein Interesse an unserem Kurs **“{{name}}”**.
{{#if direct_booking}}
Wir haben für Dich einen Platz im Kurs **verbindlich reserviert**.
{{else}}
Gerade ist ein Platz im Kurs frei geworden. Wir haben für Dich einen Platz im Kurs **verbindlich reserviert**.
{{/if}}

Die Kurstermine sind:
**{{dates}}**

Der Kurs findet statt in: **{{location}}**

{{#eq payment_type "BankTransfer"}}
Bitte überweise die Teilnahmegebühr i.H.v. **{{price}}** bis zum **{{payday}}** auf dieses Konto:

**{{payment_details}}**
{{/eq}}
{{#eq payment_type "SepaDirectDebit"}}
Die Teilnahmegebühr i.H.v. **{{price}}** wird einige Tage vor Kursbeginn per SEPA-Lastschrift von deinem Konto abgebucht.
{{/eq}}

Ein letzter Hinweis: Es gibt eine Mindestteilnehmerzahl, damit der Kurs stattfinden kann. Sollten sich zu wenige anmelden oder es aus unvorhergesehen Gründen (Krankheit Kursleitung usw.) nicht am Starttermin losgehen kann, informieren wir Dich selbstverständlich sofort. Wenn Du nichts mehr von uns hörst, beginnt der Kurs wie geplant.

Wir freuen uns auf Deine Teilnahme und wünschen Dir viel Freude beim Kurs.

Herzliche Grüße
Team Fitness@SVE"#;

    const SVETOOLS_COURSE_CANCELLATION: &str = r#"Hallo {{firstname}},

leider kann der von dir gebuchte Fitnesskurs **"{{name}}"** aufgrund zu geringer Teilnehmerzahl nicht stattfinden.

Solltest du den Kurs bereits bezahlt haben, **werden wir dir das Geld selbstverständlich umgehend zurücküberweisen**.

Wir entschuldigen uns nochmals für die Absage und hoffen, dass in Zukunft wieder ein passender Kurs für dich dabei ist.

Herzliche Grüße
Team Fitness@SVE"#;

    const SVETOOLS_PREBOOKING_INVITATION: &str = r#"Hallo {{firstname}},

wir werden deinen gebuchten Fitnesskurs "{{name}}" in Kürze zu folgenden Terminen erneut anbieten:

**{{dates}}**

Der kommende Kurs kostet **{{price}}**.
Als bisheriger Kursteilnehmer möchten wir es Dir ermöglichen, den Kurs vor der offiziellen Veröffentlichung exklusiv zu buchen.

Möchtest du wieder mit dabei sein? Dann klicke einfach auf deinen individuellen Buchungslink, um den Kurs zu buchen:
**{{link}}**

**Achtung:** Wir werden den Fitnesskurs in Kürze öffentlich ausschreiben. Ab diesem Zeitpunkt ist eine Buchung nur noch über die Webseite möglich, und ein Platz ist nicht mehr garantiert.

Herzliche Grüße
Team Fitness@SVE"#;

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
            Vec::new(),
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
            PaymentMethod::BankTransfer,
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
    fn test_render_booking_generic_bolds_but_not_text() {
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
            None,
        );
        let event = new_event(vec![Utc::now()]);

        let (_, result) = render_booking_generic(
            "Hallo {{firstname}}!\n**{{name}}** ist **ausgebucht**.\nPreis & Info <besser>",
            &booking,
            &event,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(result.contains(
            "Hallo Max!<br><strong>name</strong> ist <strong>ausgebucht</strong>.<br>Preis &amp; Info &lt;besser&gt;"
        ));
        assert!(!result.contains("**"));
        assert!(result.contains(r#"<html lang="de""#));
        assert!(result.contains("https://www.sv-eutingen.de/logo.png"));
        assert!(!result.contains("PS:"));
    }

    #[test]
    fn test_render_booking_generic_appends_ps() {
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
            None,
        );
        let event = new_event(vec![Utc::now()]);

        let (_, result) = render_booking_generic(
            "Hallo {{firstname}}!",
            &booking,
            &event,
            None,
            None,
            None,
            Some("PS: Ab sofort\nhttps://example.com/unsubscribe"),
        )
        .unwrap();

        assert!(result.contains("PS: Ab sofort<br>https://example.com/unsubscribe"));
    }

    #[test]
    fn test_render_booking_real_db_fitness_template() {
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
            None,
        );
        let mut event = new_event(vec![Utc::now()]);
        event.payment_account = Some(String::from(
            "Sportverein Eutingen im Gäu e.V.\nIBAN: DE16 6429 1010 0034 4696 05",
        ));

        let template = DB_FITNESS_TEMPLATE;

        let text = render_booking(
            template,
            &booking,
            &event,
            Some(String::from("26-1001")),
            None,
            Some(true),
        )
        .unwrap();
        assert!(text.contains("**"));
        assert!(text.contains("Kurs **“name”**."));
        assert!(text.contains("verbindlich reserviert"));
        assert!(text.contains("Der Kurs findet statt in: **location**"));
        assert!(text.contains("Verwendungszweck: 26-1001"));

        let (_, html) = render_booking_generic(
            template,
            &booking,
            &event,
            Some(String::from("26-1001")),
            None,
            Some(true),
            None,
        )
        .unwrap();
        assert!(html.contains("<strong>“name”</strong>"));
        assert!(html.contains("<strong>verbindlich reserviert</strong>"));
        assert!(html.contains("<strong>location</strong>"));
        assert!(html.contains("<strong>0,00 €</strong>"));
        assert!(html.contains("Verwendungszweck: 26-1001</strong>"));
        assert!(!html.contains("**"));
        assert!(html.contains("Ein letzter Hinweis:"));
    }

    #[test]
    fn test_render_booking_svetools_presets() {
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
            None,
        );
        let event = new_event(vec![Utc::now()]);

        let text = render_booking(
            SVETOOLS_COURSE_CANCELLATION,
            &booking,
            &event,
            None,
            None,
            Some(true),
        )
        .unwrap();
        assert!(text.contains("**"));
        assert!(text.contains("\"name\""));

        let (_, html) = render_booking_generic(
            SVETOOLS_COURSE_CANCELLATION,
            &booking,
            &event,
            None,
            None,
            Some(true),
            None,
        )
        .unwrap();
        assert!(html.contains("<strong>&quot;name&quot;</strong>"));
        assert!(html.contains(
            "<strong>werden wir dir das Geld selbstverständlich umgehend zurücküberweisen</strong>"
        ));

        let link = Some(String::from("https://www.sv-eutingen.de/buchen/abc"));
        let text = render_booking(
            SVETOOLS_PREBOOKING_INVITATION,
            &booking,
            &event,
            None,
            link.clone(),
            None,
        )
        .unwrap();
        assert!(text.contains("**"));
        assert!(text.contains("https://www.sv-eutingen.de/buchen/abc"));

        let (_, html) = render_booking_generic(
            SVETOOLS_PREBOOKING_INVITATION,
            &booking,
            &event,
            None,
            link,
            None,
            None,
        )
        .unwrap();
        assert!(html.contains("<strong>https://www.sv-eutingen.de/buchen/abc</strong>"));
        assert!(html.contains("<strong>Achtung:</strong>"));
        assert!(html.contains("<strong>0,00 €</strong>"));
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
            PaymentMethod::BankTransfer,
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
            Some(Utc::now()),
            None,
            None,
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
            PaymentMethod::BankTransfer,
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
            Some(Utc::now()),
            None,
            None,
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
            PaymentMethod::BankTransfer,
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
            None,
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
            PaymentMethod::BankTransfer,
        )
    }

    #[test]
    fn test_render_html_renders_template_by_name() {
        let result = render_html(
            "contact_confirmation",
            &ContactConfirmationTemplateData::new("Max Mustermann"),
        )
        .unwrap();

        assert!(result.contains("Hallo Max Mustermann"));
        assert!(result.contains("<!doctype html>"));
        assert!(result.contains("SV Eutingen 1947 e.V."));
    }

    #[test]
    fn test_render_html_handles_missing_variable_gracefully() {
        let result = render_html(
            "contact_confirmation",
            &serde_json::json!({ "name": "Max" }),
        )
        .unwrap();

        assert!(result.contains("Hallo Max"));
    }

    #[test]
    fn test_render_html_partials_are_rendered() {
        let result = render_html(
            "membership_application",
            &serde_json::json!({ "firstname": "Anna", "newsletter": true }),
        )
        .unwrap();

        assert!(result.contains("Hallo Anna"));
        assert!(result.contains("SV Eutingen 1947 e.V."));
    }

    #[test]
    fn test_render_html_header_logo_is_linked() {
        let result = render_html(
            "contact_confirmation",
            &ContactConfirmationTemplateData::new("Max Mustermann"),
        )
        .unwrap();

        assert!(result.contains("https://www.sv-eutingen.de/logo.png"));
        assert!(result.contains(r#"href="https://www.sv-eutingen.de/" target="_blank""#));
    }

    #[test]
    fn test_render_html_footer_links_website_and_socials() {
        let result = render_html(
            "membership_application",
            &serde_json::json!({ "firstname": "Anna", "newsletter": true }),
        )
        .unwrap();

        assert!(result.contains(r#">www.sv-eutingen.de</a>"#));
        assert!(result.contains(r#"href="https://www.sv-eutingen.de" target="_blank""#));
        assert!(result.contains(r#"href="https://www.sv-eutingen.de/newsletter" target="_blank""#));
        assert!(result.contains("fussball.de"));
        assert!(result.contains(r#"rel="noopener noreferrer""#));
        assert!(result.contains("instagram.com/sveutingen1947"));
        assert!(result.contains("facebook.com/sveutingen"));
        assert!(result.contains("youtube.com/@SVEutingeneV"));
        assert!(result.contains("linkedin.com/company/sv-eutingen-1947-e-v/"));
        assert!(result.contains("impressum"));
        assert!(result.contains("datenschutz"));
    }

    #[test]
    fn test_render_html_supports_dark_mode() {
        let result = render_html(
            "contact_confirmation",
            &ContactConfirmationTemplateData::new("Max Mustermann"),
        )
        .unwrap();

        assert!(result.contains(r#"<meta name="color-scheme" content="light dark">"#));
        assert!(result.contains(r#"<meta name="supported-color-schemes" content="light dark">"#));
        assert!(result.contains("@media (prefers-color-scheme: dark)"));
        assert!(result.contains("[data-ogsc]"));
        assert!(result.contains(".sve-text a { color: #e71b17 !important; }"));
        assert!(result.contains(".sve-foot-brand { color: #e71b17 !important; }"));
    }

    #[test]
    fn test_render_html_footer_is_fluid_and_outlook_safe() {
        let result = render_html(
            "contact_confirmation",
            &ContactConfirmationTemplateData::new("Max Mustermann"),
        )
        .unwrap();

        assert!(result.contains(r#"<div style="max-width:600px;margin:0 auto;">"#));
        assert!(result.contains(r#"<!--[if mso]><table role="presentation" border="0" cellpadding="0" cellspacing="0" align="center" width="600"><tr><td><![endif]-->"#));
        assert!(result.contains(r#"<!--[if mso]></td></tr></table><![endif]-->"#));
    }

    #[test]
    fn test_render_html_uses_presentation_tables_and_german_lang() {
        let result = render_html(
            "contact_confirmation",
            &ContactConfirmationTemplateData::new("Max Mustermann"),
        )
        .unwrap();

        assert!(result.contains(r#"<html lang="de""#));
        assert!(result.contains(
            r#"<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">"#
        ));
    }

    #[test]
    fn test_render_html_contains_preheader() {
        let result = render_html(
            "cancel_booking_events",
            &serde_json::json!({ "firstname": "Max", "name": "SVE-Sommerfest 2026" }),
        )
        .unwrap();

        assert!(result.contains("Dein gebuchtes Event wurde storniert."));
    }

    #[test]
    #[ignore = "dev tool: renders all emails into target/email-preview.html for visual review"]
    fn email_preview_gallery() {
        let cases: &[(&str, &str, serde_json::Value)] = &[
            (
                "cancel_booking_events",
                "Stornierung Event",
                serde_json::json!({ "firstname": "Max", "name": "SVE-Sommerfest 2026" }),
            ),
            (
                "cancel_booking_fitness",
                "Stornierung Kurs",
                serde_json::json!({ "firstname": "Max", "name": "Rückenfit im Frühling" }),
            ),
            (
                "contact_confirmation",
                "Kontaktbestätigung",
                serde_json::json!({ "name": "Max Mustermann" }),
            ),
            (
                "event_reminder_events",
                "Event-Erinnerung",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "SVE-Sommerfest 2026",
                    "location": "Sportgelände SV Eutingen",
                    "start_date": "Samstag, 14. Juni 2026",
                    "start_time": "15:00 Uhr"
                }),
            ),
            (
                "event_reminder_fitness",
                "Kurs-Erinnerung",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "Rückenfit im Frühling",
                    "location": "Turn- & Festhalle Eutingen",
                    "start_date": "Montag, 7. April 2026",
                    "start_time": "19:00 Uhr"
                }),
            ),
            (
                "membership_application",
                "Mitgliedsantrag (mit Newsletter)",
                serde_json::json!({ "firstname": "Anna", "newsletter": true }),
            ),
            (
                "participation_confirmation_fitness",
                "Teilnahmebestätigung",
                serde_json::json!({ "firstname": "Max", "name": "Rückenfit im Frühling" }),
            ),
            (
                "payment_reminder_events",
                "Zahlungserinnerung Event",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "SVE-Sommerfest 2026",
                    "price": "15,00 €",
                    "payment_details": "Sportverein Eutingen im Gäu e.V.\nIBAN: DE16 6429 1010 0034 4696 05\nVerwendungszweck: 26-1001"
                }),
            ),
            (
                "payment_reminder_fitness",
                "Zahlungserinnerung Kurs",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "Rückenfit im Frühling",
                    "price": "60,00 €",
                    "payment_details": "Sportverein Eutingen im Gäu e.V.\nIBAN: DE16 6429 1010 0034 4696 05\nVerwendungszweck: 26-1002"
                }),
            ),
            (
                "schedule_change_events",
                "Terminänderung Event",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "SVE-Sommerfest 2026",
                    "removed_dates": "- Do., 12. Juni 2026, 15:00 Uhr",
                    "new_dates": "- Fr., 13. Juni 2026, 15:00 Uhr"
                }),
            ),
            (
                "schedule_change_fitness",
                "Terminänderung Kurs",
                serde_json::json!({
                    "firstname": "Max",
                    "name": "Rückenfit im Frühling",
                    "removed_dates": "- Mi., 09. April 2026, 19:00 Uhr",
                    "new_dates": "- Mi., 16. April 2026, 19:00 Uhr"
                }),
            ),
            (
                "waiting_list_events",
                "Warteliste Event",
                serde_json::json!({ "firstname": "Max", "name": "SVE-Sommerfest 2026" }),
            ),
            (
                "waiting_list_fitness",
                "Warteliste Kurs",
                serde_json::json!({ "firstname": "Max", "name": "Rückenfit im Frühling" }),
            ),
        ];

        let mut body = String::from(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>SVE Email Preview</title>\
             <style>body{font-family:-apple-system,sans-serif;background:#eee;margin:0;padding:24px}\
             h1{font-size:20px}\
             .card{background:#fff;border-radius:8px;box-shadow:0 1px 4px rgba(0,0,0,.15);\
             margin:0 auto 24px;max-width:1080px;overflow:hidden}\
             .card h2{font-size:13px;margin:0;padding:10px 16px;background:#f5f5f5;\
             border-bottom:1px solid #ddd;color:#333}\
             iframe{display:block;width:100%;height:560px;border:0}\
             .duo{display:grid;grid-template-columns:1fr 1fr}\
             .duo > div + div{border-left:1px solid #ddd}\
             .duo h3{font-size:11px;margin:0;padding:6px 12px;background:#fafafa;\
             border-bottom:1px solid #eee;color:#888;text-transform:uppercase}\
             .duo pre{white-space:pre-wrap;font-family:ui-monospace,monospace;font-size:12px;\
             margin:0;padding:12px 16px;min-height:560px;max-height:640px;overflow:auto;color:#222}</style></head><body>\
             <h1>SVE Email Preview \u{2014} 13 statische + 3 DB/Batch-Vorlagen</h1>",
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
            None,
        );
        let event_events = Event::new(
            0,
            Utc::now(),
            None,
            EventType::Events,
            LifecycleStatus::Draft,
            String::from("SVE-Sommerfest 2026"),
            0,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            vec![Utc.with_ymd_and_hms(2026, 6, 14, 15, 0, 0).unwrap()],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(15).unwrap(),
            BigDecimal::from_i8(20).unwrap(),
            None,
            String::from("Sportgelände SV Eutingen"),
            String::from("booking_template"),
            Some(String::from(
                "Sportverein Eutingen im Gäu e.V.\nIBAN: DE16 6429 1010 0034 4696 05",
            )),
            None,
            None,
            false,
            Vec::new(),
            PaymentMethod::BankTransfer,
        );
        let event_fitness = Event::new(
            0,
            Utc::now(),
            None,
            EventType::Fitness,
            LifecycleStatus::Draft,
            String::from("Rückenfit im Frühling"),
            0,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            vec![Utc.with_ymd_and_hms(2026, 4, 7, 19, 0, 0).unwrap()],
            None,
            0,
            0,
            0,
            BigDecimal::from_i8(60).unwrap(),
            BigDecimal::from_i8(70).unwrap(),
            None,
            String::from("Turn- & Festhalle Eutingen"),
            String::from("booking_template"),
            Some(String::from(
                "Sportverein Eutingen im Gäu e.V.\nIBAN: DE16 6429 1010 0034 4696 05",
            )),
            None,
            None,
            false,
            Vec::new(),
            PaymentMethod::BankTransfer,
        );
        let subscription = EventSubscription::new(
            0,
            Utc::now(),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("Haupstraße 1"),
            String::from("72184 Eutingen"),
            String::from("max@mustermann.de"),
            None,
            true,
            true,
            String::from("26-1001"),
            Some(Utc::now()),
            None,
            None,
            None,
            Vec::new(),
        );
        let unpaid_booking = UnpaidEventBooking::new(
            0.into(),
            String::from("Rückenfit im Frühling"),
            0,
            Utc::now(),
            String::from("Max"),
            String::from("Mustermann"),
            String::from("max@mustermann.de"),
            BigDecimal::from_i8(60).unwrap(),
            String::from("26-1002"),
            None,
            None,
        );
        let membership_application: MembershipApplication =
            serde_json::from_value(serde_json::json!({
                "salutation": "Herr",
                "first_name": "Max",
                "last_name": "Mustermann",
                "street": "Hauptstraße 1",
                "zipcode": "72184",
                "city": "Eutingen im Gäu",
                "email": "max@mustermann.de",
                "phone": "07459 1234",
                "gender": "männlich",
                "birthday": "1990-01-01",
                "iban": "DE16 6429 1010 0034 4696 05",
                "account_owner": "Max Mustermann",
                "membership_type": "AdultActive",
                "family_members": null,
                "newsletter": true,
                "token": null
            }))
            .unwrap();
        let removed_dates = vec![Utc.with_ymd_and_hms(2026, 4, 2, 19, 0, 0).unwrap()];

        let push_duo_card = |label: &str, name: &str, html: &str, text: &str| -> String {
            let srcdoc = escape_html(html);
            let text = escape_html(text);
            format!(
                "<div class=\"card\"><h2>{label} <code style=\"color:#999\">({name})</code></h2>\
                 <div class=\"duo\"><div><h3>HTML</h3><iframe srcdoc=\"{srcdoc}\"></iframe></div>\
                 <div><h3>Text</h3><pre>{text}</pre></div></div></div>"
            )
        };

        for (name, label, data) in cases {
            let html = render_html(name, data)
                .unwrap_or_else(|e| format!("<pre>render error: {e:?}</pre>"));
            let text = match *name {
                "cancel_booking_events" => render_booking(
                    include_str!("../../templates/cancel_booking_events.txt"),
                    &booking,
                    &event_events,
                    None,
                    None,
                    None,
                ),
                "cancel_booking_fitness" => render_booking(
                    include_str!("../../templates/cancel_booking_fitness.txt"),
                    &booking,
                    &event_fitness,
                    None,
                    None,
                    None,
                ),
                "contact_confirmation" => render_contact_confirmation(
                    include_str!("../../templates/contact_confirmation.txt"),
                    "Max Mustermann",
                ),
                "event_reminder_events" => render_event_reminder(
                    include_str!("../../templates/event_reminder_events.txt"),
                    &event_events,
                    &subscription,
                ),
                "event_reminder_fitness" => render_event_reminder(
                    include_str!("../../templates/event_reminder_fitness.txt"),
                    &event_fitness,
                    &subscription,
                ),
                "membership_application" => render_membership_application(
                    include_str!("../../templates/membership_application.txt"),
                    &membership_application,
                ),
                "participation_confirmation_fitness" => render_participation_confirmation(
                    include_str!("../../templates/participation_confirmation_fitness.txt"),
                    &event_fitness,
                    &subscription,
                ),
                "payment_reminder_events" => render_payment_reminder(
                    include_str!("../../templates/payment_reminder_events.txt"),
                    &event_events,
                    &unpaid_booking,
                ),
                "payment_reminder_fitness" => render_payment_reminder(
                    include_str!("../../templates/payment_reminder_fitness.txt"),
                    &event_fitness,
                    &unpaid_booking,
                ),
                "schedule_change_events" => render_schedule_change(
                    include_str!("../../templates/schedule_change_events.txt"),
                    &booking,
                    &event_events,
                    &removed_dates,
                ),
                "schedule_change_fitness" => render_schedule_change(
                    include_str!("../../templates/schedule_change_fitness.txt"),
                    &booking,
                    &event_fitness,
                    &removed_dates,
                ),
                "waiting_list_events" => render_booking(
                    include_str!("../../templates/waiting_list_events.txt"),
                    &booking,
                    &event_events,
                    None,
                    None,
                    Some(false),
                ),
                "waiting_list_fitness" => render_booking(
                    include_str!("../../templates/waiting_list_fitness.txt"),
                    &booking,
                    &event_fitness,
                    None,
                    None,
                    Some(false),
                ),
                _ => unreachable!("no text template for {name}"),
            }
            .unwrap_or_else(|e| format!("render error: {e:?}"));
            body.push_str(&push_duo_card(label, name, &html, &text));
        }

        let render_duo = |template: &str,
                          event: &Event,
                          payment_id: Option<String>,
                          link: Option<String>,
                          direct_booking: Option<bool>,
                          ps: Option<&str>|
         -> (String, String) {
            let (text, html) = render_booking_generic(
                template,
                &booking,
                event,
                payment_id,
                link,
                direct_booking,
                ps,
            )
            .unwrap_or_else(|e| {
                let err = format!("render error: {e:?}");
                (err.clone(), err)
            });
            (html, text)
        };

        let (html, text) = render_duo(
            DB_FITNESS_TEMPLATE,
            &event_fitness,
            Some(String::from("26-1001")),
            None,
            Some(true),
            None,
        );
        body.push_str(&push_duo_card(
            "DB-Buchungsmail Fitness (echte DB-Vorlage)",
            "generic_mail",
            &html,
            &text,
        ));

        let (html, text) = render_duo(
            SVETOOLS_COURSE_CANCELLATION,
            &event_fitness,
            None,
            None,
            Some(true),
            None,
        );
        body.push_str(&push_duo_card(
            "Batch-Mail: Kursabsage (sve-tools)",
            "generic_mail",
            &html,
            &text,
        ));

        let (html, text) = render_duo(
            SVETOOLS_PREBOOKING_INVITATION,
            &event_fitness,
            None,
            Some(String::from("https://www.sv-eutingen.de/buchen/abc")),
            None,
            None,
        );
        body.push_str(&push_duo_card(
            "Prebooking-Mail: Einladung (sve-tools)",
            "generic_mail",
            &html,
            &text,
        ));

        body.push_str("</body></html>");

        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/email-preview.html");
        std::fs::write(path, &body).expect("Failed to write email preview gallery");

        println!("Email preview gallery written to {path}");
    }
}
