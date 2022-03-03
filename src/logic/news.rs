use crate::models::{NewsType, Subscription};
use crate::store::{self, GouthInterceptor};
use anyhow::Result;
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use std::collections::{HashMap, HashSet};
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

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
    // FIXME: create email

    Ok(())
}
