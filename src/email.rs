use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use crate::{
    logic::secrets,
    models::{EmailAccount, EmailType},
};
use anyhow::{Context, Result, bail};
use lettre::message::Mailbox;
use lettre::message::MessageBuilder;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::str::from_utf8;

pub(crate) fn mailbox(account: &EmailAccount) -> Result<Mailbox> {
    Ok(account.address.parse()?)
}

pub(crate) fn new_message_builder(account: &EmailAccount) -> Result<MessageBuilder> {
    Ok(Message::builder().from(mailbox(account)?).date_now())
}

pub(crate) fn create_mailer(account: &EmailAccount) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let transport = AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")?
        .credentials(Credentials::new(
            account.address.clone(),
            from_utf8(&account.password)
                .with_context(|| {
                    format!("Invalid UTF-8 sequence in password of {}", account.address)
                })?
                .into(),
        ))
        .build();
    Ok(transport)
}

pub(crate) async fn test_connection() -> Result<()> {
    let mut errors = Vec::new();
    for email_account in email_accounts().await? {
        let result = create_mailer(&email_account)?.test_connection().await;
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
    let mailer = create_mailer(from)?;
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

#[async_trait]
#[cfg_attr(test, automock)]
pub trait EmailSender {
    async fn test_connection(&self) -> Result<()>;
    async fn send_message(&self, from: &EmailAccount, message: Message) -> Result<()>;
    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()>;
    async fn get_account_by_address(&self, email_address: &str) -> Result<EmailAccount>;
    async fn get_account_by_type(&self, email_type: EmailType) -> Result<EmailAccount>;
}

pub(crate) struct RealEmailSender;

#[async_trait]
impl EmailSender for RealEmailSender {
    async fn test_connection(&self) -> Result<()> {
        test_connection().await
    }

    async fn send_message(&self, from: &EmailAccount, message: Message) -> Result<()> {
        send_message(from, message).await
    }

    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()> {
        send_messages(from, messages).await
    }

    async fn get_account_by_address(&self, email_address: &str) -> Result<EmailAccount> {
        get_account_by_address(email_address).await
    }

    async fn get_account_by_type(&self, email_type: EmailType) -> Result<EmailAccount> {
        get_account_by_type(email_type).await
    }
}
