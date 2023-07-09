use crate::models::{NewsSubscription, NewsTopic};
use crate::{db, email};
use anyhow::Result;
use lettre::message::header::{self, ContentType};
use lettre::message::SinglePart;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

pub(crate)const UNSUBSCRIBE_MESSAGE: &str = "Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:
https://www.sv-eutingen.de/newsletter";

pub(crate) async fn subscribe(pool: &PgPool, subscription: NewsSubscription) -> Result<()> {
    subscribe_to_news(pool, subscription, true).await?;

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
                .or_insert_with(HashSet::new)
                .insert(subscription.email.clone());
        }
    }

    Ok(result)
}

pub(in crate::logic) async fn subscribe_to_news(
    pool: &PgPool,
    subscription: NewsSubscription,
    send_email: bool,
) -> Result<()> {
    let subscription = db::subscribe(pool, subscription).await?;
    if send_email {
        send_mail(subscription).await?
    }

    Ok(())
}

async fn send_mail(subscription: NewsSubscription) -> Result<()> {
    let primary_news_topic;
    let multiple_topics;
    if subscription.topics.len() == 1 {
        primary_news_topic = *subscription.topics.get(0).unwrap();
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

    let email_account = email::get_account_by_type(primary_news_topic.into())?;
    let message = email_account
        .new_message()?
        .header(header::MIME_VERSION_1_0)
        .header(ContentType::TEXT_PLAIN)
        .to(subscription.email.parse()?)
        .bcc(email_account.mailbox()?)
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

    email::send_message(&email_account, message).await?;

    Ok(())
}
