use crate::models::{NewsTopic, NewsSubscription};
use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, query, query_as, Executor, FromRow, PgPool, Postgres};

const DATABASE_URL: &str = include_str!("../secrets/database_url.env");

pub async fn init_pool() -> Result<PgPool> {
    let pool = PgPoolOptions::new().connect(DATABASE_URL).await?;
    Ok(pool)
}

pub async fn get_subscriptions(pool: &PgPool) -> Result<Vec<NewsSubscription>> {
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
                return NewsSubscription::new(row.email, types);
            })
            .fetch_all(pool)
            .await?;

    Ok(subscriptions)
}

pub async fn subscribe(pool: &PgPool, subscription: NewsSubscription) -> Result<NewsSubscription> {
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
        ).execute(&mut tx)
        .await?;
    }

    tx.commit().await?;

    Ok(subscription)
}

pub async fn unsubscribe(pool: &PgPool, subscription: &NewsSubscription) -> Result<()> {
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
            .execute(&mut tx)
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

async fn get_current_subscription<'a, E>(
    executor: E,
    email: &str,
) -> Result<Option<CurrentSubscription>>
where
    E: Executor<'a, Database = Postgres>,
{
    let current_subscription: Option<CurrentSubscription> = query_as!(
        CurrentSubscription,
        r#"SELECT s.id, s.general, s.events, s.fitness FROM news_subscribers s WHERE s.email = $1"#,
        email
    )
    .fetch_optional(executor)
    .await?;

    Ok(current_subscription)
}

async fn update_subscription<'a, E>(
    executor: E,
    id: i32,
    general: bool,
    events: bool,
    fitness: bool,
) -> Result<()>
where
    E: Executor<'a, Database = Postgres>,
{
    query!(
        r#"UPDATE news_subscribers SET general = $2, events = $3, fitness = $4 WHERE id = $1"#,
        id,
        general,
        events,
        fitness
    )
    .execute(executor)
    .await?;

    Ok(())
}
