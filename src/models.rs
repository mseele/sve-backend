use anyhow::{Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bigdecimal::{BigDecimal, ParseBigDecimalError};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use lettre::Message;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::str::FromStr;

use crate::{email, hashids};

base64_serde_type!(Base64Standard, STANDARD);

/// Special i32 that is encoded / decoded to a short
/// unique id on json serialization / deserialization.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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
        Ok(EventId(id))
    }
}

impl From<i32> for EventId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub(crate) price_member: BigDecimal,
    pub(crate) price_non_member: BigDecimal,
    pub(crate) cost_per_date: Option<BigDecimal>,
    pub(crate) location: String,
    pub(crate) booking_template: String,
    pub(crate) payment_account: Option<String>,
    pub(crate) alt_booking_button_text: Option<String>,
    pub(crate) alt_email_address: Option<String>,
    pub(crate) external_operator: bool,
    pub(crate) custom_fields: Vec<EventCustomField>,
    pub(crate) payment_method: PaymentMethod,
    pub(crate) subscribers: Option<Vec<EventSubscription>>,
}

impl Event {
    #[allow(clippy::too_many_arguments)]
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
        price_member: BigDecimal,
        price_non_member: BigDecimal,
        cost_per_date: Option<BigDecimal>,
        location: String,
        booking_template: String,
        payment_account: Option<String>,
        alt_booking_button_text: Option<String>,
        alt_email_address: Option<String>,
        external_operator: bool,
        custom_fields: Vec<EventCustomField>,
        payment_method: PaymentMethod,
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
            price_member: price_member.round(2),
            price_non_member: price_non_member.round(2),
            cost_per_date: cost_per_date.map(|cost| cost.round(2)),
            location,
            booking_template,
            payment_account,
            alt_booking_button_text,
            alt_email_address,
            external_operator,
            custom_fields,
            payment_method,
            subscribers: None,
        }
    }

    pub(crate) fn price(&self, is_member: bool) -> &BigDecimal {
        match is_member {
            true => &self.price_member,
            false => &self.price_non_member,
        }
    }

    pub(crate) fn subject_prefix(&self) -> String {
        self.event_type.subject_prefix()
    }

    pub(crate) async fn get_associated_email_account(
        &self,
        email_sender: &impl email::EmailSender,
    ) -> Result<EmailAccount> {
        match &self.alt_email_address {
            Some(email_address) => email_sender.get_account_by_address(email_address).await,
            None => {
                email_sender
                    .get_account_by_type(self.event_type.into())
                    .await
            }
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
    pub(crate) price_member: Option<BigDecimal>,
    pub(crate) price_non_member: Option<BigDecimal>,
    pub(crate) cost_per_date: Option<BigDecimal>,
    pub(crate) location: Option<String>,
    pub(crate) booking_template: Option<String>,
    pub(crate) payment_account: Option<String>,
    pub(crate) alt_booking_button_text: Option<String>,
    pub(crate) alt_email_address: Option<String>,
    pub(crate) external_operator: Option<bool>,
    pub(crate) custom_fields: Option<Vec<EventCustomField>>,
    pub(crate) payment_method: Option<PaymentMethod>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "event_type")]
pub(crate) enum EventType {
    Fitness,
    Events,
}

impl EventType {
    pub(crate) fn subject_prefix(&self) -> String {
        format!(
            "[{}@SVE]",
            match self {
                EventType::Fitness => "Fitness",
                EventType::Events => "Events",
            }
        )
    }
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

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, sqlx::Type)]
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

    /// No longer visible but bookable (via rest call).
    /// Edit & Communication (Confirmation email, etc.) is still possible.
    /// No longer deletable - can only be archived by closing the event
    Running,

    /// No longer visible and no longer bookable.
    /// Communication (Confirmation email, etc.) is still possible.
    /// No longer deletable - can only be archived by closing the event
    Finished,

    /// Closed and only usable as draft for new events.
    Closed,

    /// Archived and only there for a complete history.
    Archived,
}

impl LifecycleStatus {
    pub(crate) fn is_bookable(self) -> bool {
        matches!(
            self,
            LifecycleStatus::Review | LifecycleStatus::Published | LifecycleStatus::Running
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "payment_method")]
pub(crate) enum PaymentMethod {
    BankTransfer,
    SepaDirectDebit,
}

impl FromStr for LifecycleStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "published" => Ok(Self::Published),
            "running" => Ok(Self::Running),
            "finished" => Ok(Self::Finished),
            "closed" => Ok(Self::Closed),
            "archived" => Ok(Self::Archived),
            other => bail!("Invalid lifecycle status {}", other),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "event_cf_type")]
pub(crate) enum EventCustomFieldType {
    Text,
    Number,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct EventCustomField {
    pub(crate) id: i32,
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) cf_type: EventCustomFieldType,
    pub(crate) min_value: Option<i32>,
    pub(crate) max_value: Option<i32>,
}

impl EventCustomField {
    pub(crate) fn new(
        id: i32,
        name: String,
        cf_type: EventCustomFieldType,
        min_value: Option<i32>,
        max_value: Option<i32>,
    ) -> Self {
        Self {
            id,
            name,
            cf_type,
            min_value,
            max_value,
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct EventBooking {
    pub(crate) event_id: EventId,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) street: String,
    pub(crate) city: String,
    pub(crate) email: String,
    pub(crate) phone: Option<String>,
    pub(crate) member: Option<bool>,
    pub(crate) updates: Option<bool>,
    pub(crate) comments: Option<String>,
    pub(crate) custom_values: Vec<String>,
    pub(crate) token: Option<String>,
    pub(crate) iban: Option<String>,
}

impl EventBooking {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        event_id: i32,
        first_name: String,
        last_name: String,
        street: String,
        city: String,
        email: String,
        phone: Option<String>,
        member: Option<bool>,
        updates: Option<bool>,
        comments: Option<String>,
        custom_values: Vec<String>,
        iban: Option<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            first_name,
            last_name,
            street,
            city,
            email,
            phone,
            member,
            updates,
            comments,
            custom_values,
            token: None,
            iban,
        }
    }

    pub(crate) fn is_member(&self) -> bool {
        self.member.unwrap_or(false)
    }

    pub(crate) fn price<'a>(&'a self, event: &'a Event) -> &'a BigDecimal {
        event.price(self.is_member())
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
        self.subscribers >= self.max_subscribers && self.waiting_list >= self.max_waiting_list
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct BookingResponse {
    pub(crate) success: bool,
    message: String,
    counter: Vec<EventCounter>,
    #[serde(skip_serializing_if = "is_false")]
    requires_iban: bool,
}

impl BookingResponse {
    pub(crate) fn success(message: &str, counter: Vec<EventCounter>) -> Self {
        Self {
            success: true,
            message: message.into(),
            counter,
            requires_iban: false,
        }
    }

    pub(crate) fn failure(message: &str) -> Self {
        Self {
            success: false,
            message: message.into(),
            counter: Vec::new(),
            requires_iban: false,
        }
    }

    pub(crate) fn requires_iban(message: &str) -> Self {
        Self {
            success: false,
            message: message.into(),
            counter: Vec::new(),
            requires_iban: true,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct EventSubscription {
    pub(crate) id: i32,
    pub(crate) created: DateTime<Utc>,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) street: String,
    pub(crate) city: String,
    pub(crate) email: String,
    pub(crate) phone: Option<String>,
    pub(crate) enrolled: bool,
    pub(crate) member: bool,
    pub(crate) payment_id: String,
    pub(crate) payment_confirmed_at: Option<DateTime<Utc>>,
    pub(crate) sepa_exported_at: Option<DateTime<Utc>>,
    pub(crate) iban: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) custom_values: Vec<String>,
}

impl EventSubscription {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: i32,
        created: DateTime<Utc>,
        first_name: String,
        last_name: String,
        street: String,
        city: String,
        email: String,
        phone: Option<String>,
        enrolled: bool,
        member: bool,
        payment_id: String,
        payment_confirmed_at: Option<DateTime<Utc>>,
        sepa_exported_at: Option<DateTime<Utc>>,
        iban: Option<String>,
        comment: Option<String>,
        custom_values: Vec<String>,
    ) -> Self {
        Self {
            id,
            created,
            first_name,
            last_name,
            street,
            city,
            email,
            phone,
            enrolled,
            member,
            payment_id,
            payment_confirmed_at,
            sepa_exported_at,
            iban,
            comment,
            custom_values,
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
    pub(crate) token: Option<String>,
}

impl NewsSubscription {
    pub(crate) fn new(email: String, topics: Vec<NewsTopic>) -> NewsSubscription {
        NewsSubscription {
            email,
            topics,
            token: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub(crate) enum NewsTopic {
    General,
    Events,
    Fitness,
}

impl NewsTopic {
    pub(crate) fn display_name(&self) -> &str {
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
    #[allow(clippy::too_many_arguments)]
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

#[derive(Deserialize, Clone)]
pub(crate) struct EmailAccount {
    #[serde(rename = "type")]
    pub(crate) email_type: EmailType,
    pub(crate) address: String,
    #[serde(with = "Base64Standard")]
    pub(crate) password: Vec<u8>,
}

impl EmailAccount {
    #[cfg(test)]
    pub(crate) fn new_for_test(email_type: EmailType, address: &str) -> Self {
        Self {
            email_type,
            address: address.to_string(),
            password: Vec::new(),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub(crate) enum EmailType {
    Fitness,
    Events,
    Info,
    Kunstrasen,
    Jugendturnier,
    Mitglieder,
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
    pub(crate) token: Option<String>,
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
        let message_builder = crate::email::new_message_builder(email_account)?
            .to(self.to.parse()?)
            .subject(self.subject);
        let message = match self.attachments {
            Some(attachments) => {
                let mut multi_part = MultiPart::mixed().singlepart(SinglePart::plain(self.content));
                for attachment in attachments {
                    let filename = attachment.name;
                    let content = STANDARD.decode(&attachment.data)?;
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

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Debug)]
pub(crate) struct SepaPaymentNotAllowed;

impl std::fmt::Display for SepaPaymentNotAllowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cannot update payment for SEPA bookings")
    }
}

impl std::error::Error for SepaPaymentNotAllowed {}

#[derive(Debug)]
pub(crate) enum SepaExportError {
    NotASepaEvent,
    NoBookingsAvailable,
    BicLookupFailed(String),
    ConfigIncomplete,
}

impl std::fmt::Display for SepaExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SepaExportError::NotASepaEvent => write!(f, "Event does not support SEPA direct debit"),
            SepaExportError::NoBookingsAvailable => {
                write!(f, "No bookings available for SEPA export")
            }
            SepaExportError::BicLookupFailed(msg) => write!(f, "{}", msg),
            SepaExportError::ConfigIncomplete => write!(f, "SEPA config incomplete."),
        }
    }
}

impl std::error::Error for SepaExportError {}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub(crate) struct VerifyPaymentBookingRecord {
    pub(crate) booking_id: i32,
    pub(crate) event_name: String,
    pub(crate) full_name: String,
    pub(crate) price: BigDecimal,
    pub(crate) payment_id: String,
    pub(crate) canceled: Option<DateTime<Utc>>,
    pub(crate) enrolled: bool,
    pub(crate) payment_confirmed_at: Option<DateTime<Utc>>,
}

impl VerifyPaymentBookingRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        booking_id: i32,
        event_name: String,
        full_name: String,
        price: BigDecimal,
        payment_id: String,
        canceled: Option<DateTime<Utc>>,
        enrolled: bool,
        payment_confirmed_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            booking_id,
            event_name,
            full_name,
            price: price.round(2),
            payment_id,
            canceled,
            enrolled,
            payment_confirmed_at,
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

#[derive(Serialize, Debug)]
pub(crate) struct UnpaidEventBooking {
    pub(crate) event_id: EventId,
    pub(crate) event_name: String,
    pub(crate) booking_id: i32,
    pub(crate) booking_date: DateTime<Utc>,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) email: String,
    pub(crate) price: BigDecimal,
    pub(crate) payment_id: String,
    pub(crate) due_in_days: Option<i64>,
    pub(crate) payment_reminder_sent: Option<DateTime<Utc>>,
}

impl UnpaidEventBooking {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        event_id: EventId,
        event_name: String,
        booking_id: i32,
        booking_date: DateTime<Utc>,
        first_name: String,
        last_name: String,
        email: String,
        price: BigDecimal,
        payment_id: String,
        due_in_days: Option<i64>,
        payment_reminder_sent: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            event_id,
            event_name,
            booking_id,
            booking_date,
            first_name,
            last_name,
            email,
            price: price.round(2),
            payment_id,
            due_in_days,
            payment_reminder_sent,
        }
    }
}

pub(crate) trait ToEuro {
    fn to_euro_without_symbol(&self) -> String;

    fn to_euro(&self) -> String {
        format!("{} €", self.to_euro_without_symbol())
    }
}

pub(crate) trait FromEuro {
    fn parse_euro_without_symbol(&self) -> Result<BigDecimal, ParseBigDecimalError>;
}

impl ToEuro for BigDecimal {
    fn to_euro_without_symbol(&self) -> String {
        let formatted = format!("{:.2}", self);
        formatted.replace('.', ",")
    }
}

impl FromEuro for String {
    fn parse_euro_without_symbol(&self) -> Result<BigDecimal, ParseBigDecimalError> {
        BigDecimal::from_str(&self.replace('.', "").replace(',', "."))
    }
}

impl FromEuro for &str {
    fn parse_euro_without_symbol(&self) -> Result<BigDecimal, ParseBigDecimalError> {
        self.to_string().parse_euro_without_symbol()
    }
}

#[derive(Deserialize)]
pub(crate) struct MembershipApplication {
    pub(crate) salutation: String,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) street: String,
    pub(crate) zipcode: String,
    pub(crate) city: String,
    pub(crate) email: String,
    pub(crate) phone: String,
    pub(crate) gender: String,
    pub(crate) birthday: String,
    #[serde(skip, default = "default_start_date")]
    pub(crate) start_date: NaiveDate,
    pub(crate) iban: String,
    pub(crate) account_owner: String,
    pub(crate) membership_type: MembershipType,
    pub(crate) family_members: Option<Vec<MembershipFamilyMember>>,
    pub(crate) newsletter: bool,
    pub(crate) token: Option<String>,
}

fn default_start_date() -> NaiveDate {
    Utc::now().date_naive()
}

#[derive(Deserialize)]
pub(crate) struct MembershipFamilyMember {
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) birthday: String,
}

#[derive(Deserialize)]
pub(crate) enum MembershipType {
    Fitness,
    Family,
    AdultActive,
    AdultSuporting,
    AdultPremium,
    Youth,
    Free,
}

impl MembershipType {
    pub(crate) fn get_label(&self) -> &'static str {
        match self {
            MembershipType::Fitness => "Sparte Fitness",
            MembershipType::Family => "Familienbeitrag beliebige Kinder",
            MembershipType::AdultActive => "Aktiver Erwachsener",
            MembershipType::AdultSuporting => "Fördermitglied Erwachsener",
            MembershipType::AdultPremium => "Premiummitglied Erwachsener",
            MembershipType::Youth => "Kind, Jugendliche(r)",
            MembershipType::Free => "Beitragsfrei",
        }
    }

    pub(crate) fn get_department(&self) -> &'static str {
        match self {
            MembershipType::Fitness => "Fitness",
            _ => "Hauptverein",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::FromPrimitive;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_price() {
        let member = EventBooking::new(
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
            Vec::new(),
            None,
        );
        let no_member = EventBooking::new(
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
            Vec::new(),
            None,
        );

        let event = new_event("59.0", "69");
        let price = member.price(&event);
        assert_eq!(price, &BigDecimal::from_i8(59).unwrap());
        assert_eq!(price.to_euro(), "59,00 €");
        let price = no_member.price(&event);
        assert_eq!(price, &BigDecimal::from_i8(69).unwrap());
        assert_eq!(price.to_euro(), "69,00 €");

        let event = new_event("5.99", "9.99");
        let price = member.price(&event);
        assert_eq!(price, &BigDecimal::from_str("5.99").unwrap());
        assert_eq!(price.to_euro(), "5,99 €");
        let price = no_member.price(&event);
        assert_eq!(price, &BigDecimal::from_str("9.99").unwrap());
        assert_eq!(price.to_euro(), "9,99 €");
    }

    fn new_event(price_member: &str, price_non_member: &str) -> Event {
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
            BigDecimal::from_str(price_member).unwrap(),
            BigDecimal::from_str(price_non_member).unwrap(),
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
}

#[cfg(test)]
mod additional_model_tests {
    use super::*;
    use bigdecimal::FromPrimitive;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_membership_type_get_label() {
        assert_eq!(MembershipType::Fitness.get_label(), "Sparte Fitness");
        assert_eq!(
            MembershipType::Family.get_label(),
            "Familienbeitrag beliebige Kinder"
        );
        assert_eq!(
            MembershipType::AdultActive.get_label(),
            "Aktiver Erwachsener"
        );
        assert_eq!(
            MembershipType::AdultSuporting.get_label(),
            "Fördermitglied Erwachsener"
        );
        assert_eq!(
            MembershipType::AdultPremium.get_label(),
            "Premiummitglied Erwachsener"
        );
        assert_eq!(MembershipType::Youth.get_label(), "Kind, Jugendliche(r)");
        assert_eq!(MembershipType::Free.get_label(), "Beitragsfrei");
    }

    #[test]
    fn test_membership_type_get_department() {
        assert_eq!(MembershipType::Fitness.get_department(), "Fitness");
        assert_eq!(MembershipType::Family.get_department(), "Hauptverein");
        assert_eq!(MembershipType::AdultActive.get_department(), "Hauptverein");
        assert_eq!(
            MembershipType::AdultSuporting.get_department(),
            "Hauptverein"
        );
        assert_eq!(MembershipType::AdultPremium.get_department(), "Hauptverein");
        assert_eq!(MembershipType::Youth.get_department(), "Hauptverein");
        assert_eq!(MembershipType::Free.get_department(), "Hauptverein");
    }

    #[test]
    fn test_event_counter_is_booked_up() {
        // Unlimited (-1) should never be booked up
        let counter = EventCounter::new(0, -1, 10, 100, 5);
        assert!(!counter.is_booked_up());
        // Full: subscribers >= max and waiting_list >= max_waiting
        let counter = EventCounter::new(0, 10, 5, 10, 5);
        assert!(counter.is_booked_up());
        // Partial: subscribers full but waiting list not full
        let counter = EventCounter::new(0, 10, 5, 10, 3);
        assert!(!counter.is_booked_up());
        // Partial: waiting list full but subscribers not full
        let counter = EventCounter::new(0, 10, 5, 8, 5);
        assert!(!counter.is_booked_up());
        // Empty: no subscribers, empty waiting
        let counter = EventCounter::new(0, 10, 5, 0, 0);
        assert!(!counter.is_booked_up());
    }

    #[test]
    fn test_lifecycle_status_is_bookable() {
        assert!(!LifecycleStatus::Draft.is_bookable());
        assert!(LifecycleStatus::Review.is_bookable());
        assert!(LifecycleStatus::Published.is_bookable());
        assert!(LifecycleStatus::Running.is_bookable());
        assert!(!LifecycleStatus::Finished.is_bookable());
        assert!(!LifecycleStatus::Closed.is_bookable());
        assert!(!LifecycleStatus::Archived.is_bookable());
    }

    #[test]
    fn test_event_type_subject_prefix() {
        assert_eq!(EventType::Fitness.subject_prefix(), "[Fitness@SVE]");
        assert_eq!(EventType::Events.subject_prefix(), "[Events@SVE]");
    }

    #[test]
    fn test_event_type_from_str() {
        use std::str::FromStr;
        assert_eq!(EventType::from_str("Fitness").unwrap(), EventType::Fitness);
        assert_eq!(EventType::from_str("Events").unwrap(), EventType::Events);
        assert!(EventType::from_str("Other").is_err());
        assert!(EventType::from_str("fitness").is_err());
    }

    #[test]
    fn test_lifecycle_status_from_str() {
        use std::str::FromStr;
        assert_eq!(
            LifecycleStatus::from_str("draft").unwrap(),
            LifecycleStatus::Draft
        );
        assert_eq!(
            LifecycleStatus::from_str("Draft").unwrap(),
            LifecycleStatus::Draft
        );
        assert_eq!(
            LifecycleStatus::from_str("DRAFT").unwrap(),
            LifecycleStatus::Draft
        );
        assert_eq!(
            LifecycleStatus::from_str("review").unwrap(),
            LifecycleStatus::Review
        );
        assert_eq!(
            LifecycleStatus::from_str("published").unwrap(),
            LifecycleStatus::Published
        );
        assert_eq!(
            LifecycleStatus::from_str("running").unwrap(),
            LifecycleStatus::Running
        );
        assert_eq!(
            LifecycleStatus::from_str("finished").unwrap(),
            LifecycleStatus::Finished
        );
        assert_eq!(
            LifecycleStatus::from_str("closed").unwrap(),
            LifecycleStatus::Closed
        );
        assert_eq!(
            LifecycleStatus::from_str("archived").unwrap(),
            LifecycleStatus::Archived
        );
        assert!(LifecycleStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_news_topic_display_name_and_from_str() {
        assert_eq!(NewsTopic::General.display_name(), "Allgemein");
        assert_eq!(NewsTopic::Events.display_name(), "Events");
        assert_eq!(NewsTopic::Fitness.display_name(), "Fitness");
        assert_eq!(NewsTopic::from_str("General").unwrap(), NewsTopic::General);
        assert_eq!(NewsTopic::from_str("Events").unwrap(), NewsTopic::Events);
        assert_eq!(NewsTopic::from_str("Fitness").unwrap(), NewsTopic::Fitness);
        assert!(NewsTopic::from_str("Other").is_err());
    }

    #[test]
    fn test_to_euro_formatting() {
        use bigdecimal::FromPrimitive;
        use std::str::FromStr;
        let amount = BigDecimal::from_i8(59).unwrap();
        assert_eq!(amount.to_euro_without_symbol(), "59,00");
        assert_eq!(amount.to_euro(), "59,00 €");
        let amount = BigDecimal::from_str("5.99").unwrap();
        assert_eq!(amount.to_euro_without_symbol(), "5,99");
        let amount = BigDecimal::from_i8(0).unwrap();
        assert_eq!(amount.to_euro_without_symbol(), "0,00");
        let amount = BigDecimal::from_str("1234.56").unwrap();
        assert_eq!(amount.to_euro_without_symbol(), "1234,56");
    }

    #[test]
    fn test_from_euro_parsing() {
        use std::str::FromStr;
        let s = "59,00";
        let parsed: BigDecimal = s.parse_euro_without_symbol().unwrap();
        assert_eq!(parsed, BigDecimal::from_i8(59).unwrap());
        let s = "1.234,56";
        let parsed: BigDecimal = s.parse_euro_without_symbol().unwrap();
        assert_eq!(parsed, BigDecimal::from_str("1234.56").unwrap());
        let s = "100";
        let parsed: BigDecimal = s.parse_euro_without_symbol().unwrap();
        assert_eq!(parsed, BigDecimal::from_i8(100).unwrap());
    }
}
