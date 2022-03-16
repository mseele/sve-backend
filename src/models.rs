use anyhow::{anyhow, bail, Context, Result};
use base64::STANDARD;
use chrono::{NaiveDate, NaiveDateTime};
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use std::str::from_utf8;
use std::str::FromStr;
use steel_cent::currency::EUR;
use steel_cent::formatting::{format, france_style as euro_style};
use steel_cent::Money;

base64_serde_type!(Base64Standard, STANDARD);

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

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
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

impl From<EventType> for NewsType {
    fn from(event_type: EventType) -> Self {
        match event_type {
            EventType::Fitness => NewsType::Fitness,
            EventType::Events => NewsType::Events,
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

    pub fn cost(&self, event: &Event) -> Money {
        let cost = match self.is_member() {
            true => event.cost_member,
            false => event.cost_non_member,
        };
        cost.to_euro()
    }

    pub fn cost_as_string(&self, event: &Event) -> String {
        format(euro_style(), &self.cost(event))
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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BookingResponse {
    success: bool,
    message: String,
    counter: Vec<EventCounter>,
}

impl BookingResponse {
    pub fn success(message: &str, counter: Vec<EventCounter>) -> Self {
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
pub struct Subscription {
    pub email: String,
    pub types: Vec<NewsType>,
}

impl Subscription {
    pub fn new(email: String, types: Vec<NewsType>) -> Subscription {
        Subscription { email, types }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum NewsType {
    General,
    Events,
    Fitness,
}

impl NewsType {
    pub fn display_name(self: &Self) -> &str {
        match self {
            NewsType::General => "Allgemein",
            NewsType::Events => "Events",
            NewsType::Fitness => "Fitness",
        }
    }
}

impl From<NewsType> for &str {
    fn from(news_type: NewsType) -> Self {
        match news_type {
            NewsType::General => "General",
            NewsType::Events => "Events",
            NewsType::Fitness => "Fitness",
        }
    }
}

impl FromStr for NewsType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "General" => Ok(NewsType::General),
            "Events" => Ok(NewsType::Events),
            "Fitness" => Ok(NewsType::Fitness),
            other => bail!("Invalid type {}", other),
        }
    }
}

impl From<NewsType> for EmailType {
    fn from(news_type: NewsType) -> Self {
        match news_type {
            NewsType::General => EmailType::Info,
            NewsType::Events => EmailType::Events,
            NewsType::Fitness => EmailType::Fitness,
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
        Ok(Message::builder()
            .from(self.mailbox()?)
            .date_now())
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

pub trait ToEuro {
    fn to_euro(&self) -> Money;
    fn to_euro_string(&self) -> String;
}

impl ToEuro for f64 {
    fn to_euro(&self) -> Money {
        let fract = self.fract();
        let major = (self - fract) as i64;
        let minor = (fract * 100_f64).round() as i64;
        Money::of_major_minor(EUR, major, minor)
    }

    fn to_euro_string(&self) -> String {
        format(euro_style(), &self.to_euro())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost() {
        let member = EventBooking::new(
            String::from("id"),
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
            String::from("id"),
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

        let event = new_event(59.0, 69_f64);
        let cost = member.cost(&event);
        assert_eq!(cost.major_part(), 59);
        assert_eq!(cost.minor_part(), 0);
        assert_eq!(member.cost_as_string(&event), "59,00\u{a0}€");
        let cost = no_member.cost(&event);
        assert_eq!(cost.major_part(), 69);
        assert_eq!(cost.minor_part(), 0);
        assert_eq!(no_member.cost_as_string(&event), "69,00\u{a0}€");

        let event = new_event(5.99, 9.99);
        let cost = member.cost(&event);
        assert_eq!(cost.major_part(), 5);
        assert_eq!(cost.minor_part(), 99);
        assert_eq!(member.cost_as_string(&event), "5,99\u{a0}€");
        let cost = no_member.cost(&event);
        assert_eq!(cost.major_part(), 9);
        assert_eq!(cost.minor_part(), 99);
        assert_eq!(no_member.cost_as_string(&event), "9,99\u{a0}€");
    }

    fn new_event(cost_member: f64, cost_non_member: f64) -> Event {
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
            Vec::new(),
            None,
            0,
            0,
            0,
            cost_member,
            cost_non_member,
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
