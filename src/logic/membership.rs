use super::csv;
use super::news;
use super::template;
use crate::email::EmailSender;
use crate::models::EmailAccount;
use crate::models::EmailType;
use crate::models::MembershipApplication;
use crate::models::NewsSubscription;
use crate::models::NewsTopic;
use anyhow::Context;
use anyhow::Result;
use iban::Iban;
use lettre::Message;
use lettre::message::Attachment;
use lettre::message::MultiPart;
use lettre::message::SinglePart;
use lettre::message::header::ContentType;
use sqlx::PgPool;

pub(crate) async fn application(
    pool: &PgPool,
    membership_application: MembershipApplication,
    email_sender: &impl EmailSender,
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
            email_sender,
        )
        .await?;
    }

    // send emails
    let email_account = email_sender
        .get_account_by_type(EmailType::Mitglieder)
        .await?;
    let messages = vec![
        create_welcome_email(&email_account, &membership_application)?,
        create_internal_email(&email_account, membership_application, bank_account)?,
    ];
    email_sender.send_messages(&email_account, messages).await?;

    Ok(())
}

fn create_welcome_email(
    email_account: &EmailAccount,
    membership_application: &MembershipApplication,
) -> Result<Message> {
    let template = include_str!("../../templates/membership_application.txt");
    let body = template::render_membership_application(template, membership_application)?;

    let message = crate::email::new_message_builder(email_account)?
        .to(membership_application.email.parse()?)
        .subject("Willkomen beim SV Eutingen 1947 e.V.")
        .singlepart(SinglePart::plain(body))?;

    Ok(message)
}

/// Build the HTML body for the internal membership application email
/// This is a pure function - no side effects, deterministic output
fn build_internal_email_html(
    membership_application: &MembershipApplication,
    bank_name: &str,
    bic: &str,
) -> Result<String> {
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

    Ok(body)
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

    let body = build_internal_email_html(&membership_application, &bank_name, &bic)?;

    let attachment: String = csv::write_membership_application(membership_application)?;

    let message = crate::email::new_message_builder(email_account)?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EmailType;
    use crate::test_utils::mock_email_sender_capturing_batch;
    use iban::Iban;
    use pretty_assertions::assert_eq;
    use sqlx::PgPool;

    #[test]
    fn test_build_internal_email_html_basic() {
        let ma = MembershipApplication {
            salutation: "Herr".to_string(),
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Musterstraße 1".to_string(),
            zipcode: "12345".to_string(),
            city: "Musterstadt".to_string(),
            birthday: "1990-01-01".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            phone: "1234567890".to_string(),
            email: "max@example.com".to_string(),
            membership_type: crate::models::MembershipType::AdultActive,
            account_owner: "Max Mustermann".to_string(),
            iban: "DE89370400440532013000".to_string(),
            newsletter: true,
            family_members: None,
            gender: "männlich".to_string(),
            token: None,
        };

        let html = build_internal_email_html(&ma, "Volksbank", "GENODES1FDS").unwrap();

        assert!(html.contains("<h2>Mitgliedsantrag</h2>"));
        assert!(html.contains("Herr"));
        assert!(html.contains("Max"));
        assert!(html.contains("Mustermann"));
        assert!(html.contains("Musterstraße 1"));
        assert!(html.contains("12345 Musterstadt"));
        assert!(html.contains("1990-01-01"));
        assert!(html.contains("01.01.2024"));
        assert!(html.contains("1234567890"));
        assert!(html.contains("max@example.com"));
        assert!(html.contains("Aktiver Erwachsener"));
        assert!(html.contains("Max Mustermann"));
        assert!(html.contains("DE89370400440532013000"));
        assert!(html.contains("Volksbank"));
        assert!(html.contains("GENODES1FDS"));
        assert!(!html.contains("Familienmitglieder")); // no family members
    }

    #[test]
    fn test_build_internal_email_html_with_family() {
        use crate::models::MembershipFamilyMember;

        let family_members = vec![
            MembershipFamilyMember {
                first_name: "Anna".to_string(),
                last_name: "Mustermann".to_string(),
                birthday: "1992-05-15".to_string(),
            },
            MembershipFamilyMember {
                first_name: "Ben".to_string(),
                last_name: "Mustermann".to_string(),
                birthday: "1995-08-20".to_string(),
            },
        ];

        let ma = MembershipApplication {
            salutation: "Familie".to_string(),
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Musterstraße 1".to_string(),
            zipcode: "12345".to_string(),
            city: "Musterstadt".to_string(),
            birthday: "1990-01-01".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            phone: "1234567890".to_string(),
            email: "max@example.com".to_string(),
            membership_type: crate::models::MembershipType::Family,
            account_owner: "Max Mustermann".to_string(),
            iban: "DE89370400440532013000".to_string(),
            newsletter: false,
            family_members: Some(family_members),
            gender: "männlich".to_string(),
            token: None,
        };

        let html = build_internal_email_html(&ma, "Volksbank", "GENODES1FDS").unwrap();

        assert!(html.contains("<h2>Familienmitglieder</h2>"));
        assert!(html.contains("Anna"));
        assert!(html.contains("Ben"));
        assert!(html.contains("Mustermann"));
        assert!(html.contains("1992-05-15"));
        assert!(html.contains("1995-08-20"));
        // Verify both family members appear in the table
        assert!(html.contains("<tr>\n                        <td>Anna</td>"));
        assert!(html.contains("<tr>\n                        <td>Ben</td>"));
    }

    #[sqlx::test]
    async fn test_application_with_newsletter_subscribes_and_sends_emails(pool: PgPool) {
        let membership_application = MembershipApplication {
            salutation: "Herr".to_string(),
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Musterstraße 1".to_string(),
            zipcode: "12345".to_string(),
            city: "Musterstadt".to_string(),
            birthday: "1990-01-01".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            phone: "1234567890".to_string(),
            email: "max@example.com".to_string(),
            membership_type: crate::models::MembershipType::AdultActive,
            account_owner: "Max Mustermann".to_string(),
            iban: "DE89370400440532013000".to_string(),
            newsletter: true,
            family_members: None,
            gender: "männlich".to_string(),
            token: None,
        };

        let _iban = membership_application
            .iban
            .parse::<Iban>()
            .expect("Valid IBAN");

        let (mock_sender, captured) = mock_email_sender_capturing_batch(vec![(
            EmailType::Mitglieder,
            "mitglieder@sv-eutingen.de",
        )]);

        let result = application(&pool, membership_application, &mock_sender).await;
        assert!(result.is_ok());

        let batches = captured.lock().unwrap();
        let total_messages: usize = batches.iter().map(|(_, msgs)| msgs.len()).sum();
        assert_eq!(total_messages, 2);
    }

    #[tokio::test]
    async fn test_application_without_newsletter_skips_subscription() {
        let pool = PgPool::connect_lazy("postgresql://test:test@localhost/test").unwrap();
        let membership_application = MembershipApplication {
            salutation: "Herr".to_string(),
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Musterstraße 1".to_string(),
            zipcode: "12345".to_string(),
            city: "Musterstadt".to_string(),
            birthday: "1990-01-01".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            phone: "1234567890".to_string(),
            email: "max@example.com".to_string(),
            membership_type: crate::models::MembershipType::AdultActive,
            account_owner: "Max Mustermann".to_string(),
            iban: "DE89370400440532013000".to_string(),
            newsletter: false,
            family_members: None,
            gender: "männlich".to_string(),
            token: None,
        };

        let _iban = membership_application
            .iban
            .parse::<Iban>()
            .expect("Valid IBAN");

        let (mock_sender, captured) = mock_email_sender_capturing_batch(vec![(
            EmailType::Mitglieder,
            "mitglieder@sv-eutingen.de",
        )]);

        let result = application(&pool, membership_application, &mock_sender).await;
        assert!(result.is_ok());

        let batches = captured.lock().unwrap();
        let total_messages: usize = batches.iter().map(|(_, msgs)| msgs.len()).sum();
        assert_eq!(total_messages, 2);
    }
}
