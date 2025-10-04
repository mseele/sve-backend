use crate::{
    logic::secrets,
    models::{EmailAccount, EmailType},
};
use anyhow::{Context, Result, bail};
use lettre::{AsyncTransport, Message};

pub(crate) async fn test_connection() -> Result<()> {
    let mut errors = Vec::new();
    for email_account in email_accounts().await? {
        let result = email_account.mailer()?.test_connection().await;
        match result {
            Ok(result) => {
                if !result {
                    errors.push(format!(
                        "Testing connection of {} failed: {}",
                        email_account.address, "test_connection returned false"
                    ))
                }
            }
            Err(e) => errors.push(format!(
                "Testing connection of {} failed: {}",
                email_account.address, e
            )),
        }
    }

    if !errors.is_empty() {
        bail!(
            "{} errors while testing connections:\n\n{}",
            errors.len(),
            errors.join("\n")
        );
    }

    Ok(())
}

pub(crate) async fn send_message(from: &EmailAccount, message: Message) -> Result<()> {
    send_messages(from, vec![message]).await
}

pub(crate) async fn send_messages(from: &EmailAccount, messages: Vec<Message>) -> Result<()> {
    let mailer = from.mailer()?;
    for message in messages {
        mailer.send(message).await?;
    }
    Ok(())
}

pub(crate) async fn get_account_by_address(email_address: &str) -> Result<EmailAccount> {
    let email_account = email_accounts()
        .await?
        .into_iter()
        .find(|account| account.address == email_address)
        .with_context(|| {
            format!(
                "Found no email account for email address '{}'",
                email_address
            )
        })?;
    Ok(email_account)
}

pub(crate) async fn get_account_by_type(email_type: EmailType) -> Result<EmailAccount> {
    let email_account = email_accounts()
        .await?
        .into_iter()
        .find(|account| account.email_type == email_type)
        .with_context(|| format!("Found no email account for email type {:?}", email_type))?;
    Ok(email_account)
}

async fn email_accounts() -> Result<Vec<EmailAccount>> {
    let email_accounts: Vec<EmailAccount> =
        serde_json::from_str(&secrets::get("EMAIL_ACCOUNTS").await?)?;
    Ok(email_accounts)
}
