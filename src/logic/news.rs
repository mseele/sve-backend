use crate::email;
use crate::models::{NewsType, Subscription};
use crate::store::{self, GouthInterceptor};
use anyhow::Result;
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use std::collections::{HashMap, HashSet};
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

const UNSUBSCRIBE_URL: &str = "https://www.sv-eutingen.de/newsletter#abmelden";

pub async fn subscribe(subscription: Subscription) -> Result<()> {
    let mut client = store::get_client().await?;
    subscribe_to_news(&mut client, subscription, true).await?;

    Ok(())
}

pub async fn unsubscribe(subscription: Subscription) -> Result<()> {
    let mut client = store::get_client().await?;
    store::unsubscribe(&mut client, &subscription).await?;

    Ok(())
}

pub async fn get_subscriptions() -> Result<HashMap<NewsType, HashSet<String>>> {
    let mut client = store::get_client().await?;
    let subscriptions = store::get_subscriptions(&mut client).await?;

    let mut result: HashMap<NewsType, HashSet<String>> = HashMap::new();
    for subscription in subscriptions {
        for news_type in subscription.types {
            //TODO: can we avoid clone?
            result
                .entry(news_type)
                .or_insert_with(|| HashSet::new())
                .insert(subscription.email.clone());
        }
    }

    Ok(result)
}

pub async fn subscribe_to_news(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    subscription: Subscription,
    send_email: bool,
) -> Result<()> {
    let subscription = store::subscribe(client, &subscription).await?;
    if send_email {
        send_mail(subscription).await?
    }

    Ok(())
}

async fn send_mail(subscription: Subscription) -> Result<()> {
    let primary_news_type;
    let multiple_topics;
    if subscription.types.len() == 1 {
        primary_news_type = *subscription.types.get(0).unwrap();
        multiple_topics = None
    } else {
        primary_news_type = NewsType::General;
        multiple_topics = Some(
            subscription
                .types
                .iter()
                .map(|news_type| news_type.display_name())
                .collect::<Vec<&str>>()
                .join(", "),
        );
    }
    let subject;
    let topic;
    let kind;
    let regards;
    match primary_news_type {
        NewsType::General => {
            subject = "[Fitness@SVE] Bestätigung Fitness-News Anmeldung";
            topic = "News rund um den SVE";
            if let Some(multiple_topics) = multiple_topics {
                kind = format!(" zu folgenden Themen: {}", multiple_topics);
            } else {
                kind = ", sobald es etwas neues gibt".into();
            }
            regards = "SV Eutingen";
        }
        NewsType::Events => {
            subject = "[Events@SVE] Bestätigung Event-News Anmeldung";
            topic = "unseren Events";
            kind = ", sobald neue Events online sind".into();
            regards = "Team Events@SVE";
        }
        NewsType::Fitness => {
            subject = "[Infos@SVE] Bestätigung Newsletter Anmeldung";
            topic = "unseren Fitnesskursen";
            kind = ", sobald neue Kurse online sind".into();
            regards = "Team Fitness@SVE";
        }
    };

    let email_account = email::get_account_by_type(primary_news_type.into())?;
    let message = email_account
        .new_message()?
        .to(subscription.email.parse()?)
        .bcc(email_account.mailbox()?)
        .subject(subject)
        .body(format!("Lieber Interessent/In,

        vielen Dank für Dein Interesse an {}.

        Ab sofort erhältst Du automatisch eine E-Mail{}.

        Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:
        {}

        Herzliche Grüße
        {}", topic, kind, UNSUBSCRIBE_URL, regards))?;

    email::send_message(&email_account, message).await?;

    Ok(())
}
