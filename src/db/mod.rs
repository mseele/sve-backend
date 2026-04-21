pub(crate) mod bookings;
pub(crate) mod events;
pub(crate) mod news;

pub(crate) use bookings::*;
pub(crate) use events::*;
pub(crate) use news::*;

use crate::logic::secrets;
use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub(crate) async fn init_pool() -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .idle_timeout(Some(Duration::from_secs(5 * 60)))
        .connect(&secrets::get("DATABASE_URL").await?)
        .await?;
    Ok(pool)
}
