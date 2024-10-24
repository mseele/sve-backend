use super::csv;
use super::news;
use super::template;
use crate::models::EmailAccount;
use crate::models::EmailType;
use crate::models::NewsSubscription;
use crate::models::NewsTopic;
use crate::{email, models::MembershipApplication};
use anyhow::Context;
use anyhow::Result;
use iban::Iban;
use lettre::message::header::ContentType;
use lettre::message::Attachment;
use lettre::message::MultiPart;
use lettre::message::SinglePart;
use lettre::Message;
use sqlx::PgPool;

pub(crate) async fn application(
    pool: &PgPool,
    membership_application: MembershipApplication,
) -> Result<()> {
    let bank_account = membership_application
        .iban
        .parse::<Iban>()
        .with_context(|| "Error parsing iban")?;

    // subscribe to news if newsletter is selected
    if membership_application.newsletter {
        news::subscribe_to_news(
            pool,
            NewsSubscription::new(
                membership_application.email.clone(),
                vec![NewsTopic::General],
            ),
            false,
        )
        .await?;
    }

    // send emails
    let email_account = email::get_account_by_type(EmailType::Mitglieder)?;
    let messages = vec![
        create_welcome_email(&email_account, &membership_application)?,
        create_internal_email(&email_account, membership_application, bank_account)?,
    ];
    email::send_messages(&email_account, messages).await?;

    Ok(())
}

fn create_internal_email(
    email_account: &EmailAccount,
    membership_application: MembershipApplication,
    bank_account: Iban,
) -> Result<Message> {
    let (bank_name, bic) = bank_account
        .bank_identifier()
        .and_then(fints_institute_db::get_bank_by_bank_code)
        .map(|bank| (bank.institute, bank.bic))
        .unwrap_or_else(|| ("-".into(), "-".into()));

    let mut body = format!(
        r#"
            <html>
            <head>
                <style>
                    body {{
                        font-family: Arial, sans-serif;
                        margin: 20px;
                        line-height: 1.6;
                    }}
                    table {{
                        width: 100%;
                        border-collapse: collapse;
                        margin-bottom: 20px;
                    }}
                    th, td {{
                        border: 1px solid #ccc;
                        padding: 8px;
                        text-align: left;
                    }}
                    th {{
                        background-color: #f2f2f2;
                    }}
                    h2 {{
                        color: #333;
                    }}
                </style>
            </head>
            <body>
                <h2>Mitgliedsantrag</h2>
                <table>
                    <tr>
                        <th>Anrede</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Vorname</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Nachname</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Straße / Nr</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>PLZ / Ort</th>
                        <td>{} {}</td>
                    </tr>
                    <tr>
                        <th>Geburtsdatum</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Eintrittsdatum</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Telefonnummer</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>E-Mail</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Art der Mitgliedschaft</th>
                        <td>{}</td>
                    </tr>
                </table>
        
                <h2>Bankverbindung</h2>
                <table>
                    <tr>
                        <th>Kontoinhaber</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Kreditinstitut</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>IBAN</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>BIC</th>
                        <td>{}</td>
                    </tr>
                </table>
            "#,
        &membership_application.salutation,
        &membership_application.first_name,
        &membership_application.last_name,
        &membership_application.street,
        &membership_application.zipcode,
        &membership_application.city,
        &membership_application.birthday,
        &membership_application.start_date.format("%d.%m.%Y"),
        &membership_application.phone,
        &membership_application.email,
        &membership_application.membership_type.get_label(),
        &membership_application.account_owner,
        bank_name,
        &membership_application.iban,
        bic,
    );

    if let Some(family_members) = &membership_application.family_members {
        body.push_str(
            r#"
                <h2>Familienmitglieder</h2>
                <table>
                    <tr>
                        <th>Vorname</th>
                        <th>Nachname</th>
                        <th>Geburtsdatum</th>
                    </tr>
            "#,
        );

        for family_member in family_members {
            body.push_str(&format!(
                r#"
                    <tr>
                        <td>{}</td>
                        <td>{}</td>
                        <td>{}</td>
                    </tr>
                    "#,
                family_member.first_name, family_member.last_name, family_member.birthday,
            ));
        }

        body.push_str("</table>");
    }
    body.push_str("</body></html>");
    let attachment: String = csv::write_membership_application(membership_application)?;

    let message = email_account
        .new_message()?
        .to("mitglieder@sv-eutingen.de".parse()?)
        .subject("Neuer Mitgliedsantrag")
        .multipart(
            MultiPart::mixed()
                .singlepart(SinglePart::html(body))
                .singlepart(
                    Attachment::new("mitgliedsantrag.csv".into())
                        .body(attachment, ContentType::parse("text/csv")?),
                ),
        )?;

    Ok(message)
}

fn create_welcome_email(
    email_account: &EmailAccount,
    membership_application: &MembershipApplication,
) -> Result<Message> {
    let template = include_str!("../../templates/membership_application.txt");
    let body = template::render_membership_application(template, membership_application)?;

    let message = email_account
        .new_message()?
        .to(membership_application.email.parse()?)
        .subject("Willkomen beim SV Eutingen 1947 e.V.")
        .singlepart(SinglePart::plain(body))?;

    Ok(message)
}
