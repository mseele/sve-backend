use anyhow::{bail, Context, Result};
use base64::STANDARD;
use bigdecimal::{BigDecimal, ParseBigDecimalError};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MessageBuilder, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, Message, Tokio1Executor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::str::from_utf8;
use std::str::FromStr;

use crate::hashids;

base64_serde_type!(Base64Standard, STANDARD);

/// Special i32 that is encoded / decoded to a short
/// unique id on json serialization / deserialization.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub(crate) struct EventId(i32);

impl EventId {
    pub(crate) fn get_ref(&self) -> &i32 {
        &self.0
    }

    pub(crate) fn into_inner(self) -> i32 {
        self.0
    }
}

impl Debug for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Deref for EventId {
    type Target = i32;

    fn deref(&self) -> &i32 {
        &self.0
    }
}

impl Serialize for EventId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let id = u64::try_from(self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&hashids::encode(&[id]))
    }
}

impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let ids = hashids::decode(value).map_err(serde::de::Error::custom)?;
        let id = i32::try_from(ids[0]).map_err(serde::de::Error::custom)?;
        return Ok(EventId(id));
    }
}

impl From<i32> for EventId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Event {
    pub(crate) id: EventId,
    pub(crate) created: DateTime<Utc>,
    pub(crate) closed: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub(crate) event_type: EventType,
    #[serde(rename = "status")]
    pub(crate) lifecycle_status: LifecycleStatus,
    pub(crate) name: String,
    pub(crate) sort_index: i16,
    pub(crate) short_description: String,
    pub(crate) description: String,
    pub(crate) image: String,
    pub(crate) light: bool,
    pub(crate) dates: Vec<DateTime<Utc>>,
    pub(crate) custom_date: Option<String>,
    pub(crate) duration_in_minutes: i16,
    pub(crate) max_subscribers: i16,
    pub(crate) max_waiting_list: i16,
    pub(crate) cost_member: BigDecimal,
    pub(crate) cost_non_member: BigDecimal,
    pub(crate) location: String,
    pub(crate) booking_template: String,
    pub(crate) waiting_template: String,
    pub(crate) alt_booking_button_text: Option<String>,
    pub(crate) alt_email_address: Option<String>,
    pub(crate) external_operator: bool,
}

impl Event {
    pub(crate) fn new(
        id: i32,
        created: DateTime<Utc>,
        closed: Option<DateTime<Utc>>,
        event_type: EventType,
        lifecycle_status: LifecycleStatus,
        name: String,
        sort_index: i16,
        short_description: String,
        description: String,
        image: String,
        light: bool,
        dates: Vec<DateTime<Utc>>,
        custom_date: Option<String>,
        duration_in_minutes: i16,
        max_subscribers: i16,
        max_waiting_list: i16,
        cost_member: BigDecimal,
        cost_non_member: BigDecimal,
        location: String,
        booking_template: String,
        waiting_template: String,
        alt_booking_button_text: Option<String>,
        alt_email_address: Option<String>,
        external_operator: bool,
    ) -> Self {
        Self {
            id: id.into(),
            created,
            closed,
            event_type,
            lifecycle_status,
            name,
            sort_index,
            short_description,
            description,
            image,
            light,
            dates,
            custom_date,
            duration_in_minutes,
            max_subscribers,
            max_waiting_list,
            cost_member: cost_member.round(2),
            cost_non_member: cost_non_member.round(2),
            location,
            booking_template,
            waiting_template,
            alt_booking_button_text,
            alt_email_address,
            external_operator,
        }
    }

    pub(crate) fn cost<'a>(&'a self, is_member: bool) -> &BigDecimal {
        match is_member {
            true => &self.cost_member,
            false => &self.cost_non_member,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub(crate) struct PartialEvent {
    pub(crate) id: Option<EventId>,
    pub(crate) closed: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub(crate) event_type: Option<EventType>,
    #[serde(rename = "status")]
    pub(crate) lifecycle_status: Option<LifecycleStatus>,
    pub(crate) name: Option<String>,
    pub(crate) sort_index: Option<i16>,
    pub(crate) short_description: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) light: Option<bool>,
    pub(crate) dates: Option<Vec<DateTime<Utc>>>,
    pub(crate) custom_date: Option<String>,
    pub(crate) duration_in_minutes: Option<i16>,
    pub(crate) max_subscribers: Option<i16>,
    pub(crate) max_waiting_list: Option<i16>,
    pub(crate) cost_member: Option<BigDecimal>,
    pub(crate) cost_non_member: Option<BigDecimal>,
    pub(crate) location: Option<String>,
    pub(crate) booking_template: Option<String>,
    pub(crate) waiting_template: Option<String>,
    pub(crate) alt_booking_button_text: Option<String>,
    pub(crate) alt_email_address: Option<String>,
    pub(crate) external_operator: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "event_type")]
pub(crate) enum EventType {
    Fitness,
    Events,
}

impl From<EventType> for &str {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => "Fitness",
            EventType::Events => "Events",
        }
    }
}

impl From<EventType> for NewsTopic {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => Self::Fitness,
            EventType::Events => Self::Events,
        }
    }
}

impl From<EventType> for EmailType {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => Self::Fitness,
            EventType::Events => Self::Events,
        }
    }
}

impl From<EventType> for MessageType {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => Self::Fitness,
            EventType::Events => Self::Events,
        }
    }
}

impl FromStr for EventType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Fitness" => Ok(Self::Fitness),
            "Events" => Ok(Self::Events),
            other => bail!("Invalid type {}", other),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "lifecycle_status")]
pub(crate) enum LifecycleStatus {
    /// Not visible and not bookable.
    /// Used to prepare a new event - deletion is possible
    Draft,

    /// Visible and bookable via next.sv-eutingen.de (previous beta).
    /// No longer deletable - can only be archived by closing the event
    Review,

    /// Visible and bookable via sv-eutingen.de.
    /// No longer deletable - can only be archived by closing the event
    Published,

    /// No longer visible and no longer bookable.
    /// Communication (Confirmation email, etc.) is still possible.
    /// No longer deletable - can only be archived by closing the event
    Finished,

    /// Archived and only usable as draft for new events.
    Closed,
}

impl LifecycleStatus {
    pub(crate) fn is_bookable(self) -> bool {
        match self {
            LifecycleStatus::Draft => false,
            LifecycleStatus::Review => true,
            LifecycleStatus::Published => true,
            LifecycleStatus::Finished => true,
            LifecycleStatus::Closed => false,
        }
    }
}

impl FromStr for LifecycleStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "published" => Ok(Self::Published),
            "finished" => Ok(Self::Finished),
            "closed" => Ok(Self::Closed),
            other => bail!("Invalid lifecycle status {}", other),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct EventBooking {
    pub(crate) event_id: EventId,
    pub(crate) subscriber_id: i32,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) street: String,
    pub(crate) city: String,
    pub(crate) email: String,
    pub(crate) phone: Option<String>,
    pub(crate) member: Option<bool>,
    pub(crate) updates: Option<bool>,
    pub(crate) comments: Option<String>,
}

impl EventBooking {
    pub(crate) fn new(
        event_id: i32,
        subscriber_id: i32,
        first_name: String,
        last_name: String,
        street: String,
        city: String,
        email: String,
        phone: Option<String>,
        member: Option<bool>,
        updates: Option<bool>,
        comments: Option<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            subscriber_id,
            first_name,
            last_name,
            street,
            city,
            email,
            phone,
            member,
            updates,
            comments,
        }
    }

    pub(crate) fn is_member(&self) -> bool {
        self.member.unwrap_or(false)
    }

    pub(crate) fn cost<'a>(&'a self, event: &'a Event) -> &BigDecimal {
        event.cost(self.is_member())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct EventCounter {
    pub(crate) id: EventId,
    pub(crate) max_subscribers: i16,
    pub(crate) max_waiting_list: i16,
    pub(crate) subscribers: i16,
    pub(crate) waiting_list: i16,
}

impl EventCounter {
    pub(crate) fn new(
        id: i32,
        max_subscribers: i16,
        max_waiting_list: i16,
        subscribers: i16,
        waiting_list: i16,
    ) -> Self {
        Self {
            id: id.into(),
            max_subscribers,
            max_waiting_list,
            subscribers,
            waiting_list,
        }
    }

    pub(crate) fn is_booked_up(&self) -> bool {
        if self.max_subscribers == -1 {
            return false;
        }
        return self.subscribers >= self.max_subscribers
            && self.waiting_list >= self.max_waiting_list;
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct BookingResponse {
    success: bool,
    message: String,
    counter: Vec<EventCounter>,
}

impl BookingResponse {
    pub(crate) fn success(message: &str, counter: Vec<EventCounter>) -> Self {
        Self {
            success: true,
            message: message.into(),
            counter,
        }
    }

    pub(crate) fn failure(message: &str) -> Self {
        Self {
            success: false,
            message: message.into(),
            counter: Vec::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct EventEmail {
    pub(crate) event_id: EventId,
    pub(crate) bookings: bool,
    pub(crate) waiting_list: bool,
    pub(crate) prebooking_event_id: Option<EventId>,
    pub(crate) subject: String,
    pub(crate) body: String,
    pub(crate) attachments: Option<Vec<EmailAttachment>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct NewsSubscription {
    pub(crate) email: String,
    #[serde(rename = "types")]
    pub(crate) topics: Vec<NewsTopic>,
}

impl NewsSubscription {
    pub(crate) fn new(email: String, topics: Vec<NewsTopic>) -> NewsSubscription {
        NewsSubscription { email, topics }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub(crate) enum NewsTopic {
    General,
    Events,
    Fitness,
}

impl NewsTopic {
    pub(crate) fn display_name(self: &Self) -> &str {
        match self {
            NewsTopic::General => "Allgemein",
            NewsTopic::Events => "Events",
            NewsTopic::Fitness => "Fitness",
        }
    }
}

impl From<NewsTopic> for &str {
    fn from(topic: NewsTopic) -> Self {
        match topic {
            NewsTopic::General => "General",
            NewsTopic::Events => "Events",
            NewsTopic::Fitness => "Fitness",
        }
    }
}

impl FromStr for NewsTopic {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "General" => Ok(Self::General),
            "Events" => Ok(Self::Events),
            "Fitness" => Ok(Self::Fitness),
            other => bail!("Invalid topic {}", other),
        }
    }
}

impl From<NewsTopic> for EmailType {
    fn from(topic: NewsTopic) -> Self {
        match topic {
            NewsTopic::General => Self::Info,
            NewsTopic::Events => Self::Events,
            NewsTopic::Fitness => Self::Fitness,
        }
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct Appointment {
    pub(crate) id: Option<String>,
    pub(crate) sort_index: u32,
    pub(crate) title: Option<String>,
    pub(crate) link: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) start_date: Option<NaiveDate>,
    pub(crate) end_date: Option<NaiveDate>,
    pub(crate) start_date_time: Option<NaiveDateTime>,
    pub(crate) end_date_time: Option<NaiveDateTime>,
}

impl Appointment {
    pub(crate) fn new(
        id: Option<String>,
        sort_index: u32,
        title: Option<String>,
        link: Option<String>,
        description: Option<String>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        start_date_time: Option<NaiveDateTime>,
        end_date_time: Option<NaiveDateTime>,
    ) -> Self {
        Self {
            id,
            sort_index,
            title,
            link,
            description,
            start_date,
            end_date,
            start_date_time,
            end_date_time,
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct EmailAccount {
    #[serde(rename = "type")]
    pub(crate) email_type: EmailType,
    pub(crate) name: String,
    pub(crate) address: String,
    #[serde(with = "Base64Standard")]
    password: Vec<u8>,
}

impl EmailAccount {
    pub(crate) fn mailbox(&self) -> Result<Mailbox> {
        Ok(self.address.parse()?)
    }

    pub(crate) fn new_message(&self) -> Result<MessageBuilder> {
        Ok(Message::builder().from(self.mailbox()?).date_now())
    }

    pub(crate) fn mailer(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")?
            .credentials(Credentials::new(
                self.address.clone(),
                from_utf8(&self.password)
                    .with_context(|| {
                        format!("Invalid UTF-8 sequence in password of {}", self.address)
                    })?
                    .into(),
            ))
            .build();
        Ok(transport)
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug)]
pub(crate) enum EmailType {
    Fitness,
    Events,
    Info,
    Kunstrasen,
    Jugendturnier,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ContactMessage {
    #[serde(rename = "type")]
    pub(crate) message_type: MessageType,
    pub(crate) to: String,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) phone: Option<String>,
    pub(crate) message: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Email {
    #[serde(rename = "type")]
    pub(crate) message_type: MessageType,
    pub(crate) to: String,
    pub(crate) subject: String,
    pub(crate) content: String,
    pub(crate) attachments: Option<Vec<EmailAttachment>>,
}

impl Email {
    pub(crate) fn new(
        message_type: MessageType,
        to: String,
        subject: String,
        content: String,
        attachments: Option<Vec<EmailAttachment>>,
    ) -> Self {
        Self {
            message_type,
            to,
            subject,
            content,
            attachments,
        }
    }

    pub(crate) fn into_message(self, email_account: &EmailAccount) -> Result<Message> {
        let message_builder = email_account
            .new_message()?
            .to(self.to.parse()?)
            .subject(self.subject);
        let message = match self.attachments {
            Some(attachments) => {
                let mut multi_part = MultiPart::mixed().singlepart(SinglePart::plain(self.content));
                for attachment in attachments {
                    let filename = attachment.name;
                    let content = base64::decode(&attachment.data)?;
                    let content_type = ContentType::parse(&attachment.mime_type)?;
                    multi_part = multi_part
                        .singlepart(Attachment::new(filename).body(content, content_type));
                }
                message_builder.multipart(multi_part)
            }
            None => message_builder.singlepart(SinglePart::plain(self.content)),
        }?;

        Ok(message)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct EmailAttachment {
    pub(crate) name: String,
    pub(crate) mime_type: String,
    pub(crate) data: String,
}

#[derive(Deserialize, PartialEq, Debug, Clone, Copy)]
pub(crate) enum MessageType {
    General,
    Events,
    Fitness,
    Kunstrasen,
}

impl From<MessageType> for EmailType {
    fn from(message_type: MessageType) -> Self {
        match message_type {
            MessageType::General => Self::Info,
            MessageType::Events => Self::Events,
            MessageType::Fitness => Self::Fitness,
            MessageType::Kunstrasen => Self::Kunstrasen,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct VerifyPaymentBookingRecord {
    pub(crate) booking_id: i32,
    event_name: String,
    pub(crate) full_name: String,
    pub(crate) cost: BigDecimal,
    pub(crate) payment_id: String,
    pub(crate) canceled: Option<DateTime<Utc>>,
    pub(crate) enrolled: bool,
    pub(crate) payed: Option<DateTime<Utc>>,
}

impl VerifyPaymentBookingRecord {
    pub(crate) fn new(
        booking_id: i32,
        event_name: String,
        full_name: String,
        cost: BigDecimal,
        payment_id: String,
        canceled: Option<DateTime<Utc>>,
        enrolled: bool,
        payed: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            booking_id,
            event_name,
            full_name,
            cost,
            payment_id,
            canceled,
            enrolled,
            payed,
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct VerifyPaymentResult {
    title: String,
    values: Vec<String>,
}

impl VerifyPaymentResult {
    pub(crate) fn new(title: String, values: Vec<String>) -> Self {
        Self { title, values }
    }
}

pub(crate) trait ToEuro {
    fn to_euro_without_symbol(&self) -> String;

    fn to_euro(&self) -> String {
        format!("{} €", self.to_euro_without_symbol())
    }
}

pub(crate) trait FromEuro {
    fn from_euro_without_symbol(self) -> Result<BigDecimal, ParseBigDecimalError>;
    fn from_euro_with_symbol(self) -> Result<BigDecimal, ParseBigDecimalError>;
}

impl ToEuro for BigDecimal {
    fn to_euro_without_symbol(&self) -> String {
        let formatted = format!("{:.2}", self);
        formatted.replace(".", ",")
    }
}

impl FromEuro for String {
    fn from_euro_without_symbol(self) -> Result<BigDecimal, ParseBigDecimalError> {
        BigDecimal::from_str(&self.replace(".", "").replace(",", "."))
    }

    fn from_euro_with_symbol(self) -> Result<BigDecimal, ParseBigDecimalError> {
        self.trim_end_matches("€")
            .trim_end_matches(char::is_whitespace)
            .from_euro_without_symbol()
    }
}

impl FromEuro for &str {
    fn from_euro_without_symbol(self) -> Result<BigDecimal, ParseBigDecimalError> {
        self.to_string().from_euro_without_symbol()
    }
    fn from_euro_with_symbol(self) -> Result<BigDecimal, ParseBigDecimalError> {
        self.trim_end_matches("€")
            .trim_end_matches(char::is_whitespace)
            .from_euro_without_symbol()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::FromPrimitive;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_cost() {
        let member = EventBooking::new(
            0,
            0,
            String::from("first_name"),
            String::from("last_name"),
            String::from("street"),
            String::from("city"),
            String::from("email"),
            None,
            Some(true),
            None,
            None,
        );
        let no_member = EventBooking::new(
            0,
            0,
            String::from("first_name"),
            String::from("last_name"),
            String::from("street"),
            String::from("city"),
            String::from("email"),
            None,
            None,
            None,
            None,
        );

        let event = new_event("59.0", "69");
        let cost = member.cost(&event);
        assert_eq!(cost, &BigDecimal::from_i8(59).unwrap());
        assert_eq!(cost.to_euro(), "59,00 €");
        let cost = no_member.cost(&event);
        assert_eq!(cost, &BigDecimal::from_i8(69).unwrap());
        assert_eq!(cost.to_euro(), "69,00 €");

        let event = new_event("5.99", "9.99");
        let cost = member.cost(&event);
        assert_eq!(cost, &BigDecimal::from_str("5.99").unwrap());
        assert_eq!(cost.to_euro(), "5,99 €");
        let cost = no_member.cost(&event);
        assert_eq!(cost, &BigDecimal::from_str("9.99").unwrap());
        assert_eq!(cost.to_euro(), "9,99 €");
    }

    fn new_event(cost_member: &str, cost_non_member: &str) -> Event {
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
            Vec::new(),
            None,
            0,
            0,
            0,
            BigDecimal::from_str(cost_member).unwrap(),
            BigDecimal::from_str(cost_non_member).unwrap(),
            String::from("location"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        )
    }
}
