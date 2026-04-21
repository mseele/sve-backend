use crate::db;
use crate::email::EmailSender;
use crate::models::{NewsSubscription, NewsTopic};
use anyhow::Result;
use lettre::message::SinglePart;
use lettre::message::header::{self, ContentType};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

pub(crate)const UNSUBSCRIBE_MESSAGE: &str = "Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:
https://www.sv-eutingen.de/newsletter";

pub(crate) async fn subscribe(
    pool: &PgPool,
    subscription: NewsSubscription,
    email_sender: &impl EmailSender,
) -> Result<()> {
    subscribe_to_news(pool, subscription, true, email_sender).await?;

    Ok(())
}

pub(crate) async fn unsubscribe(pool: &PgPool, subscription: NewsSubscription) -> Result<()> {
    db::unsubscribe(pool, &subscription).await?;

    Ok(())
}

pub(crate) async fn get_subscriptions(
    pool: &PgPool,
) -> Result<HashMap<NewsTopic, HashSet<String>>> {
    let subscriptions = db::get_subscriptions(pool).await?;

    let mut result: HashMap<NewsTopic, HashSet<String>> = HashMap::new();
    for subscription in subscriptions {
        for topic in subscription.topics {
            result
                .entry(topic)
                .or_default()
                .insert(subscription.email.clone());
        }
    }

    Ok(result)
}

pub(in crate::logic) async fn subscribe_to_news(
    pool: &PgPool,
    subscription: NewsSubscription,
    send_email: bool,
    email_sender: &impl EmailSender,
) -> Result<()> {
    let subscription = db::subscribe(pool, subscription).await?;
    if send_email {
        send_mail(subscription, email_sender).await?
    }

    Ok(())
}

async fn send_mail(subscription: NewsSubscription, email_sender: &impl EmailSender) -> Result<()> {
    let primary_news_topic;
    let multiple_topics;
    if subscription.topics.len() == 1 {
        primary_news_topic = *subscription.topics.first().unwrap();
        multiple_topics = None
    } else {
        primary_news_topic = NewsTopic::General;
        multiple_topics = Some(
            subscription
                .topics
                .iter()
                .map(|topic| topic.display_name())
                .collect::<Vec<&str>>()
                .join(", "),
        );
    }
    let subject;
    let topic;
    let kind;
    let regards;
    match primary_news_topic {
        NewsTopic::General => {
            subject = "[Infos@SVE] Bestätigung Newsletter Anmeldung";
            topic = "News rund um den SVE";
            if let Some(multiple_topics) = multiple_topics {
                kind = format!(" zu folgenden Themen: {}", multiple_topics);
            } else {
                kind = ", sobald es etwas neues gibt".into();
            }
            regards = "SV Eutingen";
        }
        NewsTopic::Events => {
            subject = "[Events@SVE] Bestätigung Event-News Anmeldung";
            topic = "unseren Events";
            kind = ", sobald neue Events online sind".into();
            regards = "Team Events@SVE";
        }
        NewsTopic::Fitness => {
            subject = "[Fitness@SVE] Bestätigung Fitness-News Anmeldung";
            topic = "unseren Fitnesskursen";
            kind = ", sobald neue Kurse online sind".into();
            regards = "Team Fitness@SVE";
        }
    };

    let email_account = email_sender
        .get_account_by_type(primary_news_topic.into())
        .await?;
    let message = crate::email::new_message_builder(&email_account)?
        .header(header::MIME_VERSION_1_0)
        .header(ContentType::TEXT_PLAIN)
        .to(subscription.email.parse()?)
        .bcc(crate::email::mailbox(&email_account)?)
        .subject(subject)
        .singlepart(SinglePart::plain(format!(
            "Lieber Interessent/In,

vielen Dank für Dein Interesse an {}.

Ab sofort erhältst Du automatisch eine E-Mail{}.

{}

Herzliche Grüße
{}",
            topic, kind, UNSUBSCRIBE_MESSAGE, regards
        )))?;

    email_sender.send_message(&email_account, message).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::email::MockEmailSender;
    use crate::models::{EmailType, NewsSubscription, NewsTopic};
    use crate::test_utils::mock_email_sender_capturing;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use sqlx::PgPool;

    #[sqlx::test]
    async fn test_subscribe_sends_confirmation_email(pool: PgPool) -> Result<()> {
        let (mock_sender, captured) =
            mock_email_sender_capturing(vec![(EmailType::Fitness, "fitness@sv-eutingen.de")]);

        let subscription =
            NewsSubscription::new("test@example.com".to_string(), vec![NewsTopic::Fitness]);
        subscribe(&pool, subscription, &mock_sender).await?;

        let messages = captured.lock().unwrap();
        assert!(
            !messages.is_empty(),
            "Confirmation email should have been sent"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_subscribe_to_news_no_email(pool: PgPool) -> Result<()> {
        let mut mock_sender = MockEmailSender::new();

        mock_sender
            .expect_get_account_by_type()
            .times(0)
            .returning(|_| Box::pin(async { unreachable!() }));

        mock_sender
            .expect_send_message()
            .times(0)
            .returning(|_, _| Box::pin(async { unreachable!() }));

        let subscription =
            NewsSubscription::new("test@example.com".to_string(), vec![NewsTopic::General]);
        subscribe_to_news(&pool, subscription, false, &mock_sender).await?;

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_mail_single_topic_body(pool: PgPool) -> Result<()> {
        let (mock_sender, captured) =
            mock_email_sender_capturing(vec![(EmailType::Events, "events@sv-eutingen.de")]);

        let subscription =
            NewsSubscription::new("test@example.com".to_string(), vec![NewsTopic::Events]);
        let result = db::subscribe(&pool, subscription).await?;
        send_mail(result, &mock_sender).await?;

        let messages = captured.lock().unwrap();
        let message = messages.first().expect("Email should have been sent");

        let body_string = message.formatted();
        let body = String::from_utf8_lossy(&body_string);
        let body = body.replace("=\r\n", "").replace("=\n", "");
        assert!(
            body.contains("unseren Events"),
            "Should contain Events topic text"
        );
        assert!(
            body.contains("sobald neue Events online sind"),
            "Should contain Events kind text"
        );
        assert!(
            body.contains("Team Events@SVE"),
            "Should contain Events regards"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_mail_multiple_topics_body(pool: PgPool) -> Result<()> {
        let (mock_sender, captured) =
            mock_email_sender_capturing(vec![(EmailType::Info, "info@sv-eutingen.de")]);

        let subscription = NewsSubscription::new(
            "test@example.com".to_string(),
            vec![NewsTopic::Events, NewsTopic::Fitness],
        );
        let result = db::subscribe(&pool, subscription).await?;
        send_mail(result, &mock_sender).await?;

        let messages = captured.lock().unwrap();
        let message = messages.first().expect("Email should have been sent");

        let body_string = message.formatted();
        let body = String::from_utf8_lossy(&body_string);
        let body = body.replace("=\r\n", "").replace("=\n", "");
        assert!(
            body.contains("Events"),
            "Should contain Events in topic list"
        );
        assert!(
            body.contains("Fitness"),
            "Should contain Fitness in topic list"
        );
        assert!(
            body.contains("zu folgenden Themen:"),
            "Should contain multiple topics kind text"
        );
        assert!(
            body.contains("SV Eutingen"),
            "Should contain General regards"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_unsubscribe(pool: PgPool) -> Result<()> {
        let subscription =
            NewsSubscription::new("test@example.com".to_string(), vec![NewsTopic::Fitness]);
        db::subscribe(&pool, subscription).await?;

        let unsub = NewsSubscription::new("test@example.com".to_string(), vec![NewsTopic::Fitness]);
        unsubscribe(&pool, unsub).await?;

        let subs = db::get_subscriptions(&pool).await?;
        assert!(
            subs.iter().all(|s| s.email != "test@example.com"),
            "Email should be unsubscribed"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_get_subscriptions(pool: PgPool) -> Result<()> {
        db::subscribe(
            &pool,
            NewsSubscription::new("user1@example.com".to_string(), vec![NewsTopic::Fitness]),
        )
        .await?;
        db::subscribe(
            &pool,
            NewsSubscription::new("user2@example.com".to_string(), vec![NewsTopic::Events]),
        )
        .await?;
        db::subscribe(
            &pool,
            NewsSubscription::new(
                "user3@example.com".to_string(),
                vec![NewsTopic::Fitness, NewsTopic::Events],
            ),
        )
        .await?;

        let result = get_subscriptions(&pool).await?;

        let fitness_emails = result.get(&NewsTopic::Fitness);
        assert!(fitness_emails.is_some());
        let fitness_emails = fitness_emails.unwrap();
        assert!(fitness_emails.contains("user1@example.com"));
        assert!(fitness_emails.contains("user3@example.com"));
        assert_eq!(fitness_emails.len(), 2);

        let events_emails = result.get(&NewsTopic::Events);
        assert!(events_emails.is_some());
        let events_emails = events_emails.unwrap();
        assert!(events_emails.contains("user2@example.com"));
        assert!(events_emails.contains("user3@example.com"));
        assert_eq!(events_emails.len(), 2);

        Ok(())
    }
}
