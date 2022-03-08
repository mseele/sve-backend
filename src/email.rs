use crate::models::{EmailAccount, EmailType};
use anyhow::{bail, Context, Result};
use lettre::{AsyncTransport, Message};

const EMAIL_DATA: &str = env!("SVE_EMAILS");

pub async fn test_connection() -> Result<()> {
    let mut errors = Vec::new();
    for email_account in email_accounts()? {
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

    if errors.len() > 0 {
        bail!(
            "{} errors while testing connections:\n\n{}",
            errors.len(),
            errors.join("\n")
        );
    }

    Ok(())
}

pub async fn send_message(from: &EmailAccount, message: Message) -> Result<()> {
    send_messages(from, vec![message]).await
}

pub async fn send_messages(from: &EmailAccount, messages: Vec<Message>) -> Result<()> {
    let mailer = from.mailer()?;
    for message in messages {
        mailer.send(message).await?;
    }
    Ok(())
}

pub fn get_account_by_address(email_address: &str) -> Result<EmailAccount> {
    let email_account = email_accounts()?
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

pub fn get_account_by_type(email_type: EmailType) -> Result<EmailAccount> {
    let email_account = email_accounts()?
        .into_iter()
        .find(|account| account.email_type == email_type)
        .with_context(|| format!("Found no email account for email type {:?}", email_type))?;
    Ok(email_account)
}

fn email_accounts() -> Result<Vec<EmailAccount>> {
    let email_accounts: Vec<EmailAccount> = serde_json::from_str(EMAIL_DATA)?;
    Ok(email_accounts)
}
