use crate::{
    email,
    models::{ContactMessage, MassEmail},
};
use anyhow::Result;
use log::info;

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
    // FIXME: create and send emails

    Ok(())
}
