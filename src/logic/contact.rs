use crate::models::{ContactMessage, MassEmail};
use anyhow::Result;

pub async fn message(message: ContactMessage) -> Result<()> {
    // FIXME: create and send email

    Ok(())
}

pub async fn emails(emails: Vec<MassEmail>) -> Result<()> {
    // FIXME: create and send emails

    Ok(())
}
