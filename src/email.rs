use std::str::from_utf8;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
#[cfg(test)]
use mockall::automock;

use crate::logic::secrets::{SecretKey, SecretProvider};
use crate::models::{EmailAccount, EmailType};

fn mailbox(account: &EmailAccount) -> Result<Mailbox> {
    Ok(account.address.parse()?)
}

fn new_message_builder(account: &EmailAccount) -> Result<MessageBuilder> {
    Ok(Message::builder().from(mailbox(account)?).date_now())
}

fn create_mailer(account: &EmailAccount) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
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

#[async_trait]
#[cfg_attr(test, automock)]
pub(crate) trait EmailGateway {
    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()>;
    fn build_message(&self, account: &EmailAccount) -> Result<MessageBuilder>;
    async fn account_by_type(&self, email_type: EmailType) -> Result<EmailAccount>;
}

#[derive(Clone)]
pub(crate) struct RealEmailGateway {
    secrets: Arc<dyn SecretProvider>,
}

impl RealEmailGateway {
    pub(crate) fn new(secrets: Arc<dyn SecretProvider>) -> Self {
        Self { secrets }
    }

    pub(crate) async fn test_connection(&self) -> Result<()> {
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

    async fn email_accounts(&self) -> Result<Vec<EmailAccount>> {
        let email_accounts: Vec<EmailAccount> =
            serde_json::from_str(&self.secrets.get(SecretKey::EmailAccounts).await?)?;
        Ok(email_accounts)
    }
}

#[async_trait]
impl EmailGateway for RealEmailGateway {
    async fn send_messages(&self, from: &EmailAccount, messages: Vec<Message>) -> Result<()> {
        let mailer = create_mailer(from)?;
        for message in messages {
            mailer.send(message).await?;
        }
        Ok(())
    }

    fn build_message(&self, account: &EmailAccount) -> Result<MessageBuilder> {
        new_message_builder(account)
    }

    async fn account_by_type(&self, email_type: EmailType) -> Result<EmailAccount> {
        self.email_accounts()
            .await?
            .into_iter()
            .find(|account| account.email_type == email_type)
            .with_context(|| format!("Found no email account for email type {:?}", email_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::secrets::{MockSecretProvider, SecretKey};
    use pretty_assertions::assert_eq;

    fn email_accounts_secret() -> String {
        r#"[
            {"type": "Fitness", "address": "fitness@sv-eutingen.de", "password": "cGFzc3dvcmQ="},
            {"type": "Info", "address": "info@sv-eutingen.de", "password": "cGFzc3dvcmQ="}
        ]"#
        .to_string()
    }

    #[tokio::test]
    async fn account_by_type_resolves_matching_account() {
        let mut secrets = MockSecretProvider::new();
        secrets
            .expect_get()
            .withf(|key| *key == SecretKey::EmailAccounts)
            .returning(|_| Box::pin(async { Ok(email_accounts_secret()) }));

        let gateway = RealEmailGateway::new(Arc::new(secrets));
        let account = gateway.account_by_type(EmailType::Fitness).await.unwrap();

        assert_eq!(account.email_type, EmailType::Fitness);
        assert_eq!(account.address, "fitness@sv-eutingen.de");
    }

    #[tokio::test]
    async fn account_by_type_fails_for_unknown_type() {
        let mut secrets = MockSecretProvider::new();
        secrets
            .expect_get()
            .withf(|key| *key == SecretKey::EmailAccounts)
            .returning(|_| {
                Box::pin(async {
                    Ok(r#"[{"type": "Fitness", "address": "fitness@sv-eutingen.de", "password": "cGFzc3dvcmQ="}]"#.to_string())
                })
            });

        let gateway = RealEmailGateway::new(Arc::new(secrets));
        let result = gateway.account_by_type(EmailType::Info).await;

        match result {
            Err(e) => assert!(e.to_string().contains("Found no email account")),
            Ok(_) => panic!("expected error for unknown email type"),
        }
    }

    #[test]
    fn build_message_sets_from_and_date() {
        let account = EmailAccount::new_for_test(EmailType::Info, "info@sv-eutingen.de");
        let gateway = RealEmailGateway::new(Arc::new(MockSecretProvider::new()));

        let builder = gateway.build_message(&account).unwrap();
        let message = builder
            .to("test@example.com".parse::<Mailbox>().unwrap())
            .subject("Test")
            .singlepart(lettre::message::SinglePart::plain("body".to_string()))
            .unwrap();

        assert!(
            message
                .headers()
                .get::<lettre::message::header::From>()
                .is_some()
        );
        assert!(
            message
                .headers()
                .get::<lettre::message::header::Date>()
                .is_some()
        );
    }

    #[test]
    fn mock_gateway_captures_send_messages() {
        let account = EmailAccount::new_for_test(EmailType::Info, "info@sv-eutingen.de");
        let mut mock = MockEmailGateway::new();
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_for_mock = captured.clone();
        mock.expect_send_messages().times(1).returning(
            move |account: &EmailAccount, messages: Vec<Message>| {
                captured_for_mock
                    .lock()
                    .unwrap()
                    .push((account.clone(), messages));
                Box::pin(async { Ok(()) })
            },
        );

        let message = Message::builder()
            .from(account.address.parse::<Mailbox>().unwrap())
            .to("test@example.com".parse::<Mailbox>().unwrap())
            .subject("Test")
            .singlepart(lettre::message::SinglePart::plain("body".to_string()))
            .unwrap();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(mock.send_messages(&account, vec![message]));

        assert!(result.is_ok());
        let batches = captured.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].1.len(), 1);
    }
}
