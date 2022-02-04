use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum NewsType {
    General,
    Events,
    Fitness,
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
