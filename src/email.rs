use std::str::from_utf8;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::message::MessageBuilder;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
#[cfg(test)]
use mockall::automock;

use crate::{
    logic::secrets::{SecretKey, SecretProvider},
    models::{EmailAccount, EmailType},
};

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

#[async_trait]
#[cfg_attr(test, automock)]
pub trait EmailSender {
    async fn test_connection(&self) -> Result<()>;
    async fn send_message(&self, from: &EmailAccount, message: Message) -> Result<()>;
    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()>;
    async fn get_account_by_address(&self, email_address: &str) -> Result<EmailAccount>;
    async fn get_account_by_type(&self, email_type: EmailType) -> Result<EmailAccount>;
}

#[derive(Clone)]
pub(crate) struct RealEmailSender {
    secrets: Arc<dyn SecretProvider>,
}

impl RealEmailSender {
    pub(crate) fn new(secrets: Arc<dyn SecretProvider>) -> Self {
        Self { secrets }
    }

    async fn email_accounts(&self) -> Result<Vec<EmailAccount>> {
        let email_accounts: Vec<EmailAccount> =
            serde_json::from_str(&self.secrets.get(SecretKey::EmailAccounts).await?)?;
        Ok(email_accounts)
    }
}

#[async_trait]
impl EmailSender for RealEmailSender {
    async fn test_connection(&self) -> Result<()> {
        let mut errors = Vec::new();
        for email_account in self.email_accounts().await? {
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

    async fn send_message(&self, from: &EmailAccount, message: Message) -> Result<()> {
        send_message(from, message).await
    }

    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()> {
        send_messages(from, messages).await
    }

    async fn get_account_by_address(&self, email_address: &str) -> Result<EmailAccount> {
        self.email_accounts()
            .await?
            .into_iter()
            .find(|account| account.address == email_address)
            .with_context(|| {
                format!(
                    "Found no email account for email address '{}'",
                    email_address
                )
            })
    }

    async fn get_account_by_type(&self, email_type: EmailType) -> Result<EmailAccount> {
        self.email_accounts()
            .await?
            .into_iter()
            .find(|account| account.email_type == email_type)
            .with_context(|| format!("Found no email account for email type {:?}", email_type))
    }
}
