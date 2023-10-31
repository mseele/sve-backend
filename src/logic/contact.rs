use crate::{
    email,
    models::{ContactMessage, Email, EmailType},
};
use anyhow::Result;
use lettre::message::SinglePart;
use std::collections::HashMap;
use tracing::info;

pub(crate) async fn message(contact_message: ContactMessage) -> Result<()> {
    let email_account = email::get_account_by_type(contact_message.message_type.into())?;

    let email = contact_message.email.trim();
    let mut body = format!(
        "
        Vor- und Nachname: {}
        Email: {}
        ",
        contact_message.name.trim(),
        email
    );
    if let Some(phone) = contact_message.phone {
        if !phone.trim().is_empty() {
            body.push_str(&format!("Telefon: {}\n", phone.trim()))
        }
    }
    body.push_str(&format!(
        "\nNachricht: {}\n",
        contact_message.message.trim()
    ));

    let message = email_account
        .new_message()?
        .subject(format!(
            "[Kontakt@Web] Nachricht von {}",
            contact_message.name
        ))
        .to(contact_message.to.parse()?)
        .reply_to(email.parse()?)
        .singlepart(SinglePart::plain(body))?;

    email::send_message(&email_account, message).await?;

    info!("Info message has been send successfully");

    Ok(())
}

pub(crate) async fn emails(emails: Vec<Email>) -> Result<()> {
    let mut grouped_emails: HashMap<EmailType, Vec<Email>> = HashMap::new();
    for email in emails {
        let email_type = email.message_type.into();
        grouped_emails.entry(email_type).or_default().push(email);
    }
    for (email_type, emails) in grouped_emails {
        let from = email::get_account_by_type(email_type)?;
        let messages = emails
            .into_iter()
            .map(|email| email.into_message(&from))
            .collect::<anyhow::Result<Vec<_>>>()?;
        email::send_messages(&from, messages).await?;
    }

    Ok(())
}
