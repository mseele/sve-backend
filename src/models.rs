use anyhow::{anyhow, bail, Context, Result};
use base64::STANDARD;
use bigdecimal::{BigDecimal, ParseBigDecimalError};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use google_sheets4::api::ValueRange;
use lettre::message::{Mailbox, MessageBuilder};
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
pub struct EventId(i32);

impl EventId {
    pub fn get_ref(&self) -> &i32 {
        &self.0
    }

    pub fn into_inner(self) -> i32 {
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
        EventId(value)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EventNew {
    pub id: EventId,
    pub created: DateTime<Utc>,
    pub closed: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub event_type: EventType,
    #[serde(rename = "status")]
    pub lifecycle_status: LifecycleStatus,
    pub name: String,
    pub sort_index: i16,
    pub short_description: String,
    pub description: String,
    pub image: String,
    pub light: bool,
    pub dates: Vec<DateTime<Utc>>,
    pub custom_date: Option<String>,
    pub duration_in_minutes: i16,
    pub max_subscribers: i16,
    pub max_waiting_list: i16,
    pub cost_member: BigDecimal,
    pub cost_non_member: BigDecimal,
    pub location: String,
    pub booking_template: String,
    pub waiting_template: String,
    pub alt_booking_button_text: Option<String>,
    pub alt_email_address: Option<String>,
    pub external_operator: bool,
}

impl EventNew {
    pub fn new(
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
            cost_member,
            cost_non_member,
            location,
            booking_template,
            waiting_template,
            alt_booking_button_text,
            alt_email_address,
            external_operator,
        }
    }

    pub fn cost<'a>(&'a self, is_member: bool) -> &BigDecimal {
        match is_member {
            true => &self.cost_member,
            false => &self.cost_non_member,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct PartialEventNew {
    pub id: Option<EventId>,
    pub closed: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub event_type: Option<EventType>,
    #[serde(rename = "status")]
    pub lifecycle_status: Option<LifecycleStatus>,
    pub name: Option<String>,
    pub sort_index: Option<i16>,
    pub short_description: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub light: Option<bool>,
    pub dates: Option<Vec<DateTime<Utc>>>,
    pub custom_date: Option<String>,
    pub duration_in_minutes: Option<i16>,
    pub max_subscribers: Option<i16>,
    pub max_waiting_list: Option<i16>,
    pub cost_member: Option<BigDecimal>,
    pub cost_non_member: Option<BigDecimal>,
    pub location: Option<String>,
    pub booking_template: Option<String>,
    pub waiting_template: Option<String>,
    pub alt_booking_button_text: Option<String>,
    pub alt_email_address: Option<String>,
    pub external_operator: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub sheet_id: String,
    pub gid: i64,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub name: String,
    pub sort_index: i64,
    pub visible: bool,
    pub beta: bool,
    pub short_description: String,
    pub description: String,
    pub image: String,
    pub light: bool,
    pub dates: Vec<NaiveDateTime>,
    pub custom_date: Option<String>,
    pub duration_in_minutes: i64,
    pub max_subscribers: i64,
    pub subscribers: i64,
    pub cost_member: f64,
    pub cost_non_member: f64,
    pub waiting_list: i64,
    pub max_waiting_list: i64,
    pub location: String,
    pub booking_template: String,
    pub waiting_template: String,
    pub alt_booking_button_text: Option<String>,
    pub alt_email_address: Option<String>,
    pub external_operator: bool,
}

impl Event {
    pub fn new(
        id: String,
        sheet_id: String,
        gid: i64,
        event_type: EventType,
        name: String,
        sort_index: i64,
        visible: bool,
        beta: bool,
        short_description: String,
        description: String,
        image: String,
        light: bool,
        dates: Vec<NaiveDateTime>,
        custom_date: Option<String>,
        duration_in_minutes: i64,
        max_subscribers: i64,
        subscribers: i64,
        cost_member: f64,
        cost_non_member: f64,
        waiting_list: i64,
        max_waiting_list: i64,
        location: String,
        booking_template: String,
        waiting_template: String,
        alt_booking_button_text: Option<String>,
        alt_email_address: Option<String>,
        external_operator: bool,
    ) -> Self {
        Self {
            id,
            sheet_id,
            gid,
            event_type,
            name,
            sort_index,
            visible,
            beta,
            short_description,
            description,
            image,
            light,
            dates,
            custom_date,
            duration_in_minutes,
            max_subscribers,
            subscribers,
            cost_member,
            cost_non_member,
            waiting_list,
            max_waiting_list,
            location,
            booking_template,
            waiting_template,
            alt_booking_button_text,
            alt_email_address,
            external_operator,
        }
    }

    pub fn is_booked_up(&self) -> bool {
        if self.max_subscribers == -1 {
            return false;
        }
        return self.subscribers >= self.max_subscribers
            && self.waiting_list >= self.max_waiting_list;
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PartialEvent {
    pub id: String,
    pub sheet_id: Option<String>,
    pub gid: Option<i64>,
    #[serde(rename = "type")]
    pub event_type: Option<EventType>,
    pub name: Option<String>,
    pub sort_index: Option<i64>,
    pub visible: Option<bool>,
    pub beta: Option<bool>,
    pub short_description: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub light: Option<bool>,
    pub dates: Option<Vec<NaiveDateTime>>,
    pub custom_date: Option<String>,
    pub duration_in_minutes: Option<i64>,
    pub max_subscribers: Option<i64>,
    pub subscribers: Option<i64>,
    pub cost_member: Option<f64>,
    pub cost_non_member: Option<f64>,
    pub waiting_list: Option<i64>,
    pub max_waiting_list: Option<i64>,
    pub location: Option<String>,
    pub booking_template: Option<String>,
    pub waiting_template: Option<String>,
    pub alt_booking_button_text: Option<String>,
    pub alt_email_address: Option<String>,
    pub external_operator: Option<bool>,
}

impl TryFrom<PartialEvent> for Event {
    type Error = anyhow::Error;

    fn try_from(value: PartialEvent) -> Result<Self, Self::Error> {
        return Ok(Event::new(
            value.id,
            value
                .sheet_id
                .ok_or_else(|| anyhow!("Attribute 'sheet_id' is missing"))?,
            value
                .gid
                .ok_or_else(|| anyhow!("Attribute 'gid' is missing"))?,
            value
                .event_type
                .ok_or_else(|| anyhow!("Attribute 'event_type' is missing"))?,
            value
                .name
                .ok_or_else(|| anyhow!("Attribute 'name' is missing"))?,
            value
                .sort_index
                .ok_or_else(|| anyhow!("Attribute 'sort_index' is missing"))?,
            value
                .visible
                .ok_or_else(|| anyhow!("Attribute 'visible' is missing"))?,
            value
                .beta
                .ok_or_else(|| anyhow!("Attribute 'beta' is missing"))?,
            value
                .short_description
                .ok_or_else(|| anyhow!("Attribute 'short_description' is missing"))?,
            value
                .description
                .ok_or_else(|| anyhow!("Attribute 'description' is missing"))?,
            value
                .image
                .ok_or_else(|| anyhow!("Attribute 'image' is missing"))?,
            value
                .light
                .ok_or_else(|| anyhow!("Attribute 'light' is missing"))?,
            value
                .dates
                .ok_or_else(|| anyhow!("Attribute 'dates' is missing"))?,
            value.custom_date,
            value
                .duration_in_minutes
                .ok_or_else(|| anyhow!("Attribute 'duration_in_minutes' is missing"))?,
            value
                .max_subscribers
                .ok_or_else(|| anyhow!("Attribute 'max_subscribers' is missing"))?,
            value
                .subscribers
                .ok_or_else(|| anyhow!("Attribute 'subscribers' is missing"))?,
            value
                .cost_member
                .ok_or_else(|| anyhow!("Attribute 'cost_member' is missing"))?,
            value
                .cost_non_member
                .ok_or_else(|| anyhow!("Attribute 'cost_non_member' is missing"))?,
            value
                .waiting_list
                .ok_or_else(|| anyhow!("Attribute 'waiting_list' is missing"))?,
            value
                .max_waiting_list
                .ok_or_else(|| anyhow!("Attribute 'max_waiting_list' is missing"))?,
            value
                .location
                .ok_or_else(|| anyhow!("Attribute 'location' is missing"))?,
            value
                .booking_template
                .ok_or_else(|| anyhow!("Attribute 'booking_template' is missing"))?,
            value
                .waiting_template
                .ok_or_else(|| anyhow!("Attribute 'waiting_template' is missing"))?,
            value.alt_booking_button_text,
            value.alt_email_address,
            value
                .external_operator
                .ok_or_else(|| anyhow!("Attribute 'external_operator' is missing"))?,
        ));
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "event_type")]
pub enum EventType {
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
            EventType::Fitness => NewsTopic::Fitness,
            EventType::Events => NewsTopic::Events,
        }
    }
}

impl From<EventType> for EmailType {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => EmailType::Fitness,
            EventType::Events => EmailType::Events,
        }
    }
}

impl FromStr for EventType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Fitness" => Ok(EventType::Fitness),
            "Events" => Ok(EventType::Events),
            other => bail!("Invalid type {}", other),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "lifecycle_status")]
pub enum LifecycleStatus {
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventBookingNew {
    pub event_id: EventId,
    pub first_name: String,
    pub last_name: String,
    pub street: String,
    pub city: String,
    pub email: String,
    pub phone: Option<String>,
    pub member: Option<bool>,
    pub updates: Option<bool>,
    pub comments: Option<String>,
}

impl EventBookingNew {
    pub fn new(
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
        }
    }

    pub fn is_member(&self) -> bool {
        self.member.unwrap_or(false)
    }

    pub fn cost<'a>(&'a self, event: &'a EventNew) -> &BigDecimal {
        event.cost(self.is_member())
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventBooking {
    pub event_id: String,
    pub first_name: String,
    pub last_name: String,
    pub street: String,
    pub city: String,
    pub email: String,
    pub phone: Option<String>,
    pub member: Option<bool>,
    pub updates: Option<bool>,
    pub comments: Option<String>,
}

impl EventBooking {
    pub fn new(
        event_id: String,
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
            event_id,
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

    pub fn is_member(&self) -> bool {
        self.member.unwrap_or(false)
    }

    pub fn cost(&self, event: &Event) -> f64 {
        match self.is_member() {
            true => event.cost_member,
            false => event.cost_non_member,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventCounter {
    pub id: String,
    pub max_subscribers: i64,
    pub subscribers: i64,
    pub waiting_list: i64,
    pub max_waiting_list: i64,
}

impl EventCounter {
    pub fn new(
        id: String,
        max_subscribers: i64,
        subscribers: i64,
        waiting_list: i64,
        max_waiting_list: i64,
    ) -> Self {
        Self {
            id,
            max_subscribers,
            subscribers,
            waiting_list,
            max_waiting_list,
        }
    }
}

impl From<Event> for EventCounter {
    fn from(event: Event) -> Self {
        EventCounter::new(
            event.id,
            event.max_subscribers,
            event.subscribers,
            event.waiting_list,
            event.max_waiting_list,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventCounterNew {
    pub id: EventId,
    pub max_subscribers: i16,
    pub max_waiting_list: i16,
    pub subscribers: i16,
    pub waiting_list: i16,
}

impl EventCounterNew {
    pub fn new(
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

    pub fn is_booked_up(&self) -> bool {
        if self.max_subscribers == -1 {
            return false;
        }
        return self.subscribers >= self.max_subscribers
            && self.waiting_list >= self.max_waiting_list;
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BookingResponse {
    success: bool,
    message: String,
    counter: Vec<EventCounterNew>,
}

impl BookingResponse {
    pub fn success(message: &str, counter: Vec<EventCounterNew>) -> Self {
        Self {
            success: true,
            message: message.into(),
            counter,
        }
    }

    pub fn failure(message: &str) -> Self {
        Self {
            success: false,
            message: message.into(),
            counter: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsSubscription {
    pub email: String,
    #[serde(rename = "types")]
    pub topics: Vec<NewsTopic>,
}

impl NewsSubscription {
    pub fn new(email: String, topics: Vec<NewsTopic>) -> NewsSubscription {
        NewsSubscription { email, topics }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum NewsTopic {
    General,
    Events,
    Fitness,
}

impl NewsTopic {
    pub fn display_name(self: &Self) -> &str {
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
            "General" => Ok(NewsTopic::General),
            "Events" => Ok(NewsTopic::Events),
            "Fitness" => Ok(NewsTopic::Fitness),
            other => bail!("Invalid topic {}", other),
        }
    }
}

impl From<NewsTopic> for EmailType {
    fn from(topic: NewsTopic) -> Self {
        match topic {
            NewsTopic::General => EmailType::Info,
            NewsTopic::Events => EmailType::Events,
            NewsTopic::Fitness => EmailType::Fitness,
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Appointment {
    pub id: Option<String>,
    pub sort_index: u32,
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub start_date_time: Option<NaiveDateTime>,
    pub end_date_time: Option<NaiveDateTime>,
}

impl Appointment {
    pub fn new(
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
#[serde(rename_all = "camelCase")]
pub struct EmailAccount {
    #[serde(rename = "type")]
    pub email_type: EmailType,
    pub name: String,
    pub address: String,
    #[serde(with = "Base64Standard")]
    password: Vec<u8>,
}

impl EmailAccount {
    pub fn mailbox(&self) -> Result<Mailbox> {
        Ok(self.address.parse()?)
    }

    pub fn new_message(&self) -> Result<MessageBuilder> {
        Ok(Message::builder().from(self.mailbox()?).date_now())
    }

    pub fn mailer(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
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
pub enum EmailType {
    Fitness,
    Events,
    Info,
    Kunstrasen,
    Jugendturnier,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContactMessage {
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub to: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub message: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MassEmails {
    pub emails: Vec<MassEmail>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MassEmail {
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub to: String,
    pub subject: String,
    pub content: String,
    pub attachments: Option<Vec<EmailAttachment>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EmailAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: String,
}

#[derive(Deserialize, PartialEq, Debug, Clone, Copy)]
pub enum MessageType {
    General,
    Events,
    Fitness,
    Kunstrasen,
}

impl From<MessageType> for EmailType {
    fn from(message_type: MessageType) -> Self {
        match message_type {
            MessageType::General => EmailType::Info,
            MessageType::Events => EmailType::Events,
            MessageType::Fitness => EmailType::Fitness,
            MessageType::Kunstrasen => EmailType::Kunstrasen,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct VerifyPaymentBookingRecordNew {
    pub booking_id: i32,
    event_name: String,
    pub full_name: String,
    pub cost: BigDecimal,
    pub payment_id: String,
    pub canceled: Option<DateTime<Utc>>,
    pub enrolled: bool,
    pub payed: Option<DateTime<Utc>>,
}

impl VerifyPaymentBookingRecordNew {
    pub fn new(
        booking_id: i32,
        event_name: String,
        full_name: String,
        cost: BigDecimal,
        payment_id: String,
        canceled: Option<DateTime<Utc>>,
        enrolled: bool,
        payed: Option<DateTime<Utc>>,
    ) -> Self {
        VerifyPaymentBookingRecordNew {
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

#[derive(Debug, PartialEq)]
pub struct VerifyPaymentBookingRecord {
    sheet_title: String,
    update_cell: String,
    pub cost: BigDecimal,
    pub booking_number: String,
    pub payed_already: bool,
}

impl VerifyPaymentBookingRecord {
    pub fn new(
        sheet_title: String,
        update_cell: String,
        cost: BigDecimal,
        booking_number: String,
        payed_already: bool,
    ) -> Self {
        VerifyPaymentBookingRecord {
            sheet_title,
            update_cell,
            cost,
            booking_number,
            payed_already,
        }
    }

    pub fn into_value_range(self) -> ValueRange {
        ValueRange {
            values: Some(vec![vec![String::from("J")]]),
            range: Some(format!("'{}'!{}", self.sheet_title, self.update_cell)),
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct VerifyPaymentResult {
    title: String,
    values: Vec<String>,
}

impl VerifyPaymentResult {
    pub fn new(title: String, values: Vec<String>) -> Self {
        VerifyPaymentResult { title, values }
    }
}

pub trait ToEuro {
    fn to_euro_without_symbol(&self) -> String;

    fn to_euro(&self) -> String {
        format!("{} €", &self.to_euro_without_symbol())
    }
}

pub trait FromEuro {
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
        let member = EventBookingNew::new(
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
        let no_member = EventBookingNew::new(
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

    fn new_event(cost_member: &str, cost_non_member: &str) -> EventNew {
        EventNew::new(
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
