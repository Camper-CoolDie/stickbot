mod config;
mod context;
mod credentials;
mod event;
mod service;
pub mod strings {
    include!(concat!(env!("OUT_DIR"), "/strings.rs"));
}

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::{Error, Result};
use bonfire::Client;
use bonfire::fcm::Client as FcmClient;
use bonfire::models::Config as BonfireConfig;
pub use config::Config;
pub use context::Context;
pub use credentials::Credentials;
pub use event::Event;
pub use service::Service;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .try_init()?;

    let config = Config::load()?;

    let connect_options = SqliteConnectOptions::new()
        .filename("stickbot.db")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(connect_options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let (client, fcm_client, maybe_subscription, initial_data) =
        if let Some(credentials) = Credentials::load()? {
            let client = Client::builder().auth(credentials.auth)?.build();
            let fcm_client = FcmClient::builder().credentials(credentials.fcm)?.build();
            let initial_data = client.get_initial_data().await?;

            (client, fcm_client, credentials.subscription, initial_data)
        } else {
            let client = Client::default();
            let fcm_client = FcmClient::default();

            let (fcm_credentials, initial_data) = tokio::try_join!(
                // Forces FcmClient to register
                async { fcm_client.credentials().await.map_err(Error::from) },
                async {
                    client
                        .login(&config.bonfire.email, &config.bonfire.password)
                        .await?;
                    client.get_initial_data().await.map_err(Error::from)
                },
            )?;

            let credentials = Credentials {
                // Client should have fresh auth by now, so auth() is instant
                auth: client.auth().await?,
                fcm: fcm_credentials,
                subscription: None,
            };

            credentials.save()?;
            (client, fcm_client, None, initial_data)
        };

    let chat_cache = VecDeque::with_capacity(Service::CHAT_CACHE_SIZE);
    let ctx = Context {
        config: Arc::new(config),
        client,
        b_config: Arc::new(BonfireConfig {
            protoadmin_ids: initial_data.protoadmin_ids,
            has_follows: initial_data.has_follows,
            ..Default::default()
        }),
        account: Arc::new(initial_data.account),
        settings: Arc::new(initial_data.settings),
        fcm_client,
        pool,
        cancellation_token: CancellationToken::new(),
        chat_cache: Arc::new(Mutex::new(chat_cache)),
    };

    let subscription = Service::listen(&ctx, maybe_subscription).await?;

    ctx.pool.close().await;
    Credentials {
        auth: ctx.client.auth().await?,
        fcm: ctx.fcm_client.credentials().await?,
        subscription: Some(subscription),
    }
    .save()?;
    Ok(())
}
