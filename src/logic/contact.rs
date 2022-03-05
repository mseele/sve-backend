use crate::{
    email,
    models::{ContactMessage, EmailAccount, EmailType, MassEmail},
};
use anyhow::Result;
use lettre::{
    message::{header::ContentType, Attachment, MultiPart, SinglePart},
    Message,
};
use log::info;
use std::collections::HashMap;

pub async fn message(contact_message: ContactMessage) -> Result<()> {
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
        if phone.trim().len() > 0 {
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
        .body(body)?;

    email::send_message(&email_account, message).await?;

    info!("Info message has been send successfully");

    Ok(())
}

pub async fn emails(emails: Vec<MassEmail>) -> Result<()> {
    let mut grouped_emails: HashMap<EmailType, Vec<MassEmail>> = HashMap::new();
    for email in emails {
        let email_type = email.message_type.into();
        grouped_emails
            .entry(email_type)
            .or_insert_with(|| Vec::new())
            .push(email);
    }
    for (email_type, emails) in grouped_emails {
        let from = email::get_account_by_type(email_type)?;
        let messages = emails
            .into_iter()
            .map(|email| map(&from, email))
            .collect::<anyhow::Result<Vec<_>>>()?;
        email::send_messages(&from, messages).await?;
    }

    Ok(())
}

fn map(email_account: &EmailAccount, email: MassEmail) -> Result<Message> {
    let message_builder = email_account
        .new_message()?
        .to(email.to.parse()?)
        .subject(email.subject);
    let message = match email.attachments {
        Some(attachments) => {
            let mut multi_part = MultiPart::mixed().singlepart(SinglePart::plain(email.content));
            for attachment in attachments {
                let filename = attachment.name;
                let content = base64::decode(&attachment.data)?;
                let content_type = ContentType::parse(&attachment.mime_type)?;
                multi_part =
                    multi_part.singlepart(Attachment::new(filename).body(content, content_type));
            }
            message_builder.multipart(multi_part)
        }
        None => message_builder.body(email.content),
    }?;

    Ok(message)
}
