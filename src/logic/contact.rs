use crate::email::EmailSender;
use crate::models::{ContactMessage, Email, EmailType};
use anyhow::Result;
use lettre::message::SinglePart;
use std::collections::HashMap;
use tracing::info;

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
    email_sender: &impl EmailSender,
) -> Result<()> {
    let email_account = email_sender
        .get_account_by_type(contact_message.message_type.into())
        .await?;

    let body = build_contact_body(&contact_message);

    let message = email_account
        .new_message()?
        .subject(format!(
            "[Kontakt@Web] Nachricht von {}",
            contact_message.name
        ))
        .to(contact_message.to.parse()?)
        .reply_to(contact_message.email.parse()?)
        .singlepart(SinglePart::plain(body))?;

    email_sender.send_message(&email_account, message).await?;

    info!("Info message has been send successfully");

    Ok(())
}

pub(crate) async fn emails(emails: Vec<Email>, email_sender: &impl EmailSender) -> Result<()> {
    let mut grouped_emails: HashMap<EmailType, Vec<Email>> = HashMap::new();
    for email in emails {
        let email_type = email.message_type.into();
        grouped_emails.entry(email_type).or_default().push(email);
    }
    for (email_type, emails) in grouped_emails {
        let from = email_sender.get_account_by_type(email_type).await?;
        let messages = emails
            .into_iter()
            .map(|email| email.into_message(&from))
            .collect::<anyhow::Result<Vec<_>>>()?;
        email_sender.send_messages(&from, messages).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EmailType;
    use crate::test_utils::{mock_email_sender_capturing, mock_email_sender_capturing_batch};
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

        let (mock_sender, captured) = mock_email_sender_capturing_batch(vec![
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
    async fn test_message_sends_to_correct_account() {
        let (mock_sender, captured) =
            mock_email_sender_capturing(vec![(EmailType::Info, "info@sv-eutingen.de")]);

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
        assert_eq!(sent.len(), 1, "One email should have been sent");
    }
}
