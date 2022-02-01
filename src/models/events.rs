use anyhow::{anyhow, bail, Error};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub sheet_id: String,
    pub gid: i64,
    #[serde(rename = "type")]
    pub kind: Kind,
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
        kind: Kind,
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
    ) -> Event {
        Event {
            id,
            sheet_id,
            gid,
            kind,
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
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PartialEvent {
    pub id: String,
    pub sheet_id: Option<String>,
    pub gid: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<Kind>,
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
                .kind
                .ok_or_else(|| anyhow!("Attribute 'kind' is missing"))?,
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename = "EventType")]
pub enum Kind {
    Fitness,
    Events,
}

impl From<Kind> for &str {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Fitness => "Fitness",
            Kind::Events => "Events",
        }
    }
}

impl FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Fitness" => Ok(Kind::Fitness),
            "Events" => Ok(Kind::Events),
            other => bail!("Invalid type {}", other),
        }
    }
}
