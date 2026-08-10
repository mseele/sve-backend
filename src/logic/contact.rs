use std::collections::HashMap;
use std::net::IpAddr;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use lettre::message::SinglePart;
#[cfg(test)]
use mockall::automock;
use tracing::info;

use crate::email::EmailGateway;
use crate::logic::template;
use crate::models::{ContactMessage, Email, EmailType};

/// Errors raised by a [`CaptchaVerifier`].
///
/// The variant distinguishes *who is at fault* so route handlers can map back to
/// the status code hCaptcha's documented failure modes warrant:
/// - [`CaptchaError::Invalid`] — the submitted token did not validate. Client
///   fault, maps to `400 BAD_REQUEST`.
/// - [`CaptchaError::Internal`] — request construction, remote-IP handling, or
///   the outbound verification call broke. Server fault, maps to
///   `500 INTERNAL_SERVER_ERROR`.
#[derive(Debug)]
pub(crate) enum CaptchaError {
    Invalid(anyhow::Error),
    Internal(anyhow::Error),
}

impl std::fmt::Display for CaptchaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptchaError::Invalid(err) => write!(f, "captcha invalid: {err}"),
            CaptchaError::Internal(err) => write!(f, "captcha verification failed: {err}"),
        }
    }
}

impl std::error::Error for CaptchaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CaptchaError::Invalid(err) | CaptchaError::Internal(err) => Some(err.as_ref()),
        }
    }
}

#[cfg_attr(test, automock)]
#[async_trait]
pub(crate) trait CaptchaVerifier: Send + Sync {
    async fn verify(&self, token: &str, ip: Option<IpAddr>) -> Result<(), CaptchaError>;
}

#[derive(Clone)]
pub(crate) struct HcaptchaVerifier {
    secret: String,
}

impl HcaptchaVerifier {
    pub(crate) fn new(secret: String) -> Self {
        Self { secret }
    }
}

#[async_trait]
impl CaptchaVerifier for HcaptchaVerifier {
    async fn verify(&self, token: &str, ip: Option<IpAddr>) -> Result<(), CaptchaError> {
        let captcha = hcaptcha::Captcha::new(token)
            .map_err(|e| CaptchaError::Invalid(anyhow!("Failed to create captcha: {e:?}")))?;
        let mut request = hcaptcha::Request::new(&self.secret, captcha).map_err(|e| {
            CaptchaError::Internal(anyhow!("Failed to build captcha request: {e:?}"))
        })?;
        if let Some(ip) = ip {
            request = request.set_remoteip(&ip.to_string()).map_err(|e| {
                CaptchaError::Internal(anyhow!("Failed to set captcha remote ip: {e:?}"))
            })?;
        }
        let response = hcaptcha::Client::new()
            .verify(request)
            .await
            .map_err(|e| CaptchaError::Internal(anyhow!("Captcha verification failed: {e:?}")))?;
        if response.success() {
            Ok(())
        } else {
            Err(CaptchaError::Invalid(anyhow!(
                "Captcha invalid: {:?}",
                response.error_codes()
            )))
        }
    }
}

/// Build the plain text body for a contact message
fn build_contact_body(contact_message: &ContactMessage) -> String {
    let email = contact_message.email.trim();
    let mut body = format!(
        "Vor- und Nachname: {}\nEmail: {}\n",
        contact_message.name.trim(),
        email
    );
    if let Some(phone) = &contact_message.phone {
        let phone = phone.trim();
        if !phone.is_empty() {
            body.push_str(&format!("Telefon: {}\n", phone));
        }
    }
    body.push_str(&format!(
        "\nNachricht: {}\n",
        contact_message.message.trim()
    ));
    body
}

pub(crate) async fn message(
    contact_message: ContactMessage,
    email_gateway: &impl EmailGateway,
) -> Result<()> {
    let email_account = email_gateway
        .account_by_type(contact_message.message_type.into())
        .await?;

    let body = build_contact_body(&contact_message);

    let message = email_gateway
        .build_message(&email_account)?
        .subject(format!(
            "[Kontakt@Web] Nachricht von {}",
            contact_message.name
        ))
        .to(contact_message.to.parse()?)
        .reply_to(contact_message.email.parse()?)
        .singlepart(SinglePart::plain(body))?;

    email_gateway
        .send_messages(&email_account, vec![message])
        .await?;

    info!("Info message has been send successfully");

    let confirmation_body = template::render_contact_confirmation(
        include_str!("../../templates/contact_confirmation.txt"),
        &contact_message.name,
    )?;

    let confirmation_html = template::render_contact_confirmation_html(&contact_message.name)?;

    let confirmation = Email::new(
        contact_message.message_type,
        contact_message.email.clone(),
        "Vielen Dank für Deine Nachricht".to_string(),
        confirmation_body,
        None,
    )
    .with_html(confirmation_html)
    .into_message(&email_account, email_gateway)?;

    email_gateway
        .send_messages(&email_account, vec![confirmation])
        .await?;

    info!("Confirmation email has been sent successfully");

    Ok(())
}

pub(crate) async fn emails(emails: Vec<Email>, email_gateway: &impl EmailGateway) -> Result<()> {
    let mut grouped_emails: HashMap<EmailType, Vec<Email>> = HashMap::new();
    for email in emails {
        let email_type = email.message_type.into();
        grouped_emails.entry(email_type).or_default().push(email);
    }
    for (email_type, emails) in grouped_emails {
        let from = email_gateway.account_by_type(email_type).await?;
        let messages = emails
            .into_iter()
            .map(|email| email.into_message(&from, email_gateway))
            .collect::<anyhow::Result<Vec<_>>>()?;
        email_gateway.send_messages(&from, messages).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EmailType;
    use crate::test_utils::mock_email_gateway;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_build_contact_body_all_fields() {
        let cm = ContactMessage {
            name: "  Max Mustermann  ".to_string(),
            email: "  max@example.com  ".to_string(),
            phone: Some("  12345  ".to_string()),
            message: "  Hallo!  ".to_string(),
            message_type: crate::models::MessageType::General,
            to: "info@example.com".to_string(),
            token: None,
        };

        let body = build_contact_body(&cm);

        assert_eq!(
            body,
            "Vor- und Nachname: Max Mustermann\nEmail: max@example.com\nTelefon: 12345\n\nNachricht: Hallo!\n"
        );
    }

    #[test]
    fn test_build_contact_body_missing_phone() {
        let cm = ContactMessage {
            name: "Max Mustermann".to_string(),
            email: "max@example.com".to_string(),
            phone: None,
            message: "Test message".to_string(),
            message_type: crate::models::MessageType::General,
            to: "info@example.com".to_string(),
            token: None,
        };

        let body = build_contact_body(&cm);

        assert_eq!(
            body,
            "Vor- und Nachname: Max Mustermann\nEmail: max@example.com\n\nNachricht: Test message\n"
        );
    }

    #[test]
    fn test_build_contact_body_empty_phone() {
        let cm = ContactMessage {
            name: "Max".to_string(),
            email: "max@example.com".to_string(),
            phone: Some("   ".to_string()),
            message: "Hello".to_string(),
            message_type: crate::models::MessageType::General,
            to: "info@example.com".to_string(),
            token: None,
        };

        let body = build_contact_body(&cm);

        assert!(!body.contains("Telefon:"));
    }

    #[tokio::test]
    async fn test_emails_groups_by_type_and_sends() {
        let emails_vec = vec![
            Email::new(
                crate::models::MessageType::General,
                "recipient1@example.com".to_string(),
                "Test 1".to_string(),
                "Body 1".to_string(),
                None,
            ),
            Email::new(
                crate::models::MessageType::General,
                "recipient2@example.com".to_string(),
                "Test 2".to_string(),
                "Body 2".to_string(),
                None,
            ),
            Email::new(
                crate::models::MessageType::Events,
                "recipient3@example.com".to_string(),
                "Event".to_string(),
                "Event body".to_string(),
                None,
            ),
        ];

        let (mock_sender, captured) = mock_email_gateway(vec![
            (EmailType::Info, "info@sv-eutingen.de"),
            (EmailType::Events, "events@sv-eutingen.de"),
        ]);

        let result = emails(emails_vec, &mock_sender).await;
        assert!(result.is_ok());

        let batches = captured.lock().unwrap();
        assert_eq!(batches.len(), 2);

        let info_batch = batches
            .iter()
            .find(|(a, _)| a.address == "info@sv-eutingen.de")
            .unwrap();
        assert_eq!(info_batch.1.len(), 2);

        let events_batch = batches
            .iter()
            .find(|(a, _)| a.address == "events@sv-eutingen.de")
            .unwrap();
        assert_eq!(events_batch.1.len(), 1);
    }

    #[tokio::test]
    async fn test_message_sends_forward_and_confirmation() {
        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Info, "info@sv-eutingen.de")]);

        let contact_message = ContactMessage {
            name: "Max Mustermann".to_string(),
            email: "max@example.com".to_string(),
            phone: Some("12345".to_string()),
            message: "Test message".to_string(),
            message_type: crate::models::MessageType::General,
            to: "info@sv-eutingen.de".to_string(),
            token: None,
        };

        let result = message(contact_message, &mock_sender).await;
        assert!(result.is_ok());

        let sent = captured.lock().unwrap();
        assert_eq!(sent.len(), 2, "Two emails should have been sent");
    }
}
