use anyhow::{bail, Context, Result};
use lettre::{transport::smtp::Error, AsyncTransport, Message};

use crate::models::{EmailAccount, EmailType};

const EMAIL_DATA: &str = include_str!("../data/email.json");

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
//     let fitness_account = email_accounts()
//         .into_iter()
//         .find(|a| match a.email_type {
//             EmailType::Fitness => true,
//             _ => false,
//         })
//         .unwrap();

//     let mailer = fitness_account.mailer()?;

//     let email = Message::builder()
//         .from(fitness_account.address.parse()?)
//         .to("mseele@gmail.com".parse()?)
//         .subject("Example subject")
//         .body(String::from("Hello, world!"))?;

//     let result = mailer.send(email).await?;
//     println!("{:?}", result);

//     // let result = connection.test_connection().await?;
//     // println!("{:?}", result);

//     Ok(())
    todo!()
}

pub fn get_account_by_address(email_address: &str) -> Result<Option<EmailAccount>> {
    Ok(email_accounts()?
        .into_iter()
        .find(|account| account.address == email_address))
}

pub fn get_account_by_type(email_type: EmailType) -> Result<Option<EmailAccount>> {
    Ok(email_accounts()?
        .into_iter()
        .find(|account| account.email_type == email_type))
}

fn email_accounts() -> Result<Vec<EmailAccount>> {
    let email_accounts: Vec<EmailAccount> = serde_json::from_str(EMAIL_DATA)?;
    Ok(email_accounts)
}
