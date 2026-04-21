use crate::models::{NewsSubscription, NewsTopic};
use anyhow::Result;
use sqlx::{FromRow, PgConnection, PgPool, query, query_as};

pub(crate) async fn get_subscriptions(pool: &PgPool) -> Result<Vec<NewsSubscription>> {
    let subscriptions =
        query!(r#"SELECT s.email, s.general, s.events, s.fitness FROM news_subscribers s"#)
            .map(|row| {
                let mut types = Vec::new();
                if row.general {
                    types.push(NewsTopic::General);
                }
                if row.events {
                    types.push(NewsTopic::Events);
                }
                if row.fitness {
                    types.push(NewsTopic::Fitness);
                }
                NewsSubscription::new(row.email, types)
            })
            .fetch_all(pool)
            .await?;

    Ok(subscriptions)
}

pub(crate) async fn subscribe(
    pool: &PgPool,
    subscription: NewsSubscription,
) -> Result<NewsSubscription> {
    let mut tx = pool.begin().await?;
    let current_subscription = get_current_subscription(&mut tx, &subscription.email).await?;

    let general = subscription.topics.contains(&NewsTopic::General);
    let events = subscription.topics.contains(&NewsTopic::Events);
    let fitness = subscription.topics.contains(&NewsTopic::Fitness);

    if let Some(current_subscriptions) = current_subscription {
        update_subscription(
            &mut tx,
            current_subscriptions.id,
            current_subscriptions.general || general,
            current_subscriptions.events || events,
            current_subscriptions.fitness || fitness,
        )
        .await?;
    } else {
        query!(
            r#"INSERT INTO news_subscribers (email, general, events, fitness) VALUES($1, $2, $3, $4)"#,
            &subscription.email,
            general,
            events,
            fitness
        ).execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(subscription)
}

pub(crate) async fn unsubscribe(pool: &PgPool, subscription: &NewsSubscription) -> Result<()> {
    let mut tx = pool.begin().await?;

    let current_subscription = get_current_subscription(&mut tx, &subscription.email).await?;

    if let Some(current_subscription) = current_subscription {
        let general =
            current_subscription.general && !subscription.topics.contains(&NewsTopic::General);
        let events =
            current_subscription.events && !subscription.topics.contains(&NewsTopic::Events);
        let fitness =
            current_subscription.fitness && !subscription.topics.contains(&NewsTopic::Fitness);
        if general || events || fitness {
            update_subscription(&mut tx, current_subscription.id, general, events, fitness).await?;
        } else {
            query!(
                r#"DELETE FROM news_subscribers WHERE id = $1"#,
                current_subscription.id
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

#[derive(FromRow)]
struct CurrentSubscription {
    id: i32,
    general: bool,
    events: bool,
    fitness: bool,
}

async fn get_current_subscription(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<CurrentSubscription>> {
    let current_subscription: Option<CurrentSubscription> = query_as!(
        CurrentSubscription,
        r#"SELECT s.id, s.general, s.events, s.fitness FROM news_subscribers s WHERE s.email = $1"#,
        email
    )
    .fetch_optional(conn)
    .await?;

    Ok(current_subscription)
}

async fn update_subscription(
    conn: &mut PgConnection,
    id: i32,
    general: bool,
    events: bool,
    fitness: bool,
) -> Result<()> {
    query!(
        r#"UPDATE news_subscribers SET general = $2, events = $3, fitness = $4 WHERE id = $1"#,
        id,
        general,
        events,
        fitness
    )
    .execute(conn)
    .await?;

    Ok(())
}

#[cfg(test)]
mod news_integration_tests {
    use super::*;
    use crate::models::NewsTopic;
    use anyhow::Result;
    use sqlx::query;

    #[sqlx::test]
    async fn test_subscribe_and_unsubscribe(pool: PgPool) -> Result<()> {
        let subscription = crate::models::NewsSubscription::new(
            "test@example.com".to_string(),
            vec![NewsTopic::General, NewsTopic::Events],
        );

        let result = subscribe(&pool, subscription.clone()).await?;
        assert_eq!(result.email, "test@example.com");
        assert_eq!(result.topics.len(), 2);

        let unsubscribe_sub = crate::models::NewsSubscription::new(
            "test@example.com".to_string(),
            vec![NewsTopic::General],
        );
        unsubscribe(&pool, &unsubscribe_sub).await?;

        let subscriptions = get_subscriptions(&pool).await?;
        let sub = subscriptions
            .iter()
            .find(|s| s.email == "test@example.com")
            .unwrap();
        assert!(!sub.topics.contains(&NewsTopic::General));
        assert!(sub.topics.contains(&NewsTopic::Events));

        let full_unsub =
            crate::models::NewsSubscription::new("test@example.com".to_string(), vec![]);
        unsubscribe(&pool, &full_unsub).await?;

        let subscriptions = get_subscriptions(&pool).await?;
        let sub = subscriptions
            .iter()
            .find(|s| s.email == "test@example.com")
            .unwrap();
        assert!(sub.topics.contains(&NewsTopic::Events));

        Ok(())
    }

    #[sqlx::test]
    async fn test_get_subscriptions(pool: PgPool) -> Result<()> {
        let email1 = "alice@example.com";
        let email2 = "bob@example.com";

        query!(
            r#"INSERT INTO news_subscribers (email, general, events, fitness) VALUES ($1, $2, $3, $4)"#,
            email1, true, false, true
        )
        .execute(&pool)
        .await?;

        query!(
            r#"INSERT INTO news_subscribers (email, general, events, fitness) VALUES ($1, $2, $3, $4)"#,
            email2, false, true, false
        )
        .execute(&pool)
        .await?;

        let subscriptions = get_subscriptions(&pool).await?;
        assert_eq!(subscriptions.len(), 2);

        let alice = subscriptions.iter().find(|s| s.email == email1).unwrap();
        assert!(alice.topics.contains(&NewsTopic::General));
        assert!(!alice.topics.contains(&NewsTopic::Events));
        assert!(alice.topics.contains(&NewsTopic::Fitness));

        let bob = subscriptions.iter().find(|s| s.email == email2).unwrap();
        assert!(!bob.topics.contains(&NewsTopic::General));
        assert!(bob.topics.contains(&NewsTopic::Events));
        assert!(!bob.topics.contains(&NewsTopic::Fitness));

        Ok(())
    }
}
