use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use bonfire::fcm::models::Subscription;
use bonfire::models::chat::ListChatMessagesAnchor;
use bonfire::models::publication::chat_message::CreateChatMessageOptions;
use bonfire::models::{Chat, ChatMessage, ChatTag, Direct, NotificationParser, Publication};
use futures::StreamExt as _;
use tokio::signal;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::instrument;

use crate::{Context, Event, strings};

const MAX_TASKS: usize = 8;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Service;
impl Service {
    pub const CHAT_CACHE_SIZE: usize = 100;

    pub async fn listen(
        ctx: &Context,
        maybe_subscription: Option<Subscription>,
    ) -> Result<Subscription> {
        let subscription = if let Some(subscription) = maybe_subscription {
            subscription
        } else {
            ctx.fcm_client.subscribe(ctx.account.id).await?
        };
        ctx.client.set_push_token(&subscription.push_token).await?;

        let (parser, mut subscription_receiver) = NotificationParser::new();
        let mut stream = ctx
            .fcm_client
            .listen(parser, subscription, ctx.cancellation_token.clone(), 3)
            .await?;

        let mut event_counter = 0;
        let mut tasks = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(MAX_TASKS));
        loop {
            tokio::select! {
                biased;

                _ = signal::ctrl_c() => {
                    tracing::info!("interruption signal received, stopping");
                    ctx.cancellation_token.cancel()
                }
                result = &mut subscription_receiver => {
                    let (subscription, maybe_error) = result.context("parser has been dropped")?;

                    // Wait for all tasks to finish and prevent new dispatches
                    semaphore.close();
                    timeout(DRAIN_TIMEOUT, async {
                        while tasks.join_next().await.is_some() {}
                    })
                    .await
                    .context("timed out waiting for tasks to finish")?;

                    return maybe_error.map_or(Ok(subscription), |error| Err(error.into()));
                }
                maybe_result = stream.next() => {
                    let Some(result) = maybe_result else {
                        ctx.cancellation_token.cancel();
                        continue;
                    };
                    let Ok(notification) = result else {
                        continue;
                    };
                    let Some(event) = Event::new(notification) else {
                        continue;
                    };

                    let ctx = ctx.clone();
                    let semaphore = Arc::clone(&semaphore);
                    tasks.spawn(async move {
                        // acquire() can only fail when close() is called, and that's an indication
                        // that we're finishing
                        let permit = semaphore.acquire().await;
                        if permit.is_ok() {
                            Self::dispatch(&ctx, event, event_counter).await;
                        }
                    });
                    event_counter += 1;
                }
            }
        }
    }

    #[instrument(skip_all, fields(id = event_id))]
    async fn dispatch(ctx: &Context, event: Event, event_id: u64) {
        if let Err(error) = match event {
            Event::ChatMessage(message) => Self::handle_chat_message(ctx, *message).await,
            Event::ChatRead(ids) => Self::handle_chat_read(ctx, ids).await,
        } {
            tracing::error!(?error, "failed to handle event");
        }
    }

    async fn handle_chat_message(ctx: &Context, message: Publication<ChatMessage>) -> Result<()> {
        // TODO: chat messages
        Ok(())
    }

    async fn handle_chat_read(ctx: &Context, ids: (u64, u64)) -> Result<()> {
        let recipient_id = if ctx.account.id == ids.0 {
            ids.1
        } else {
            ids.0
        };

        let mut cache = ctx.chat_cache.lock().await;
        if !cache.contains(&recipient_id) {
            if cache.len() >= Self::CHAT_CACHE_SIZE {
                cache.pop_front();
            }
            cache.push_back(recipient_id);
            drop(cache);

            let chat = Chat::<Direct>::new(ids);
            let mut messages =
                chat.list_messages(&ctx.client, true, &ListChatMessagesAnchor::Newest);

            if messages.next().await.transpose()?.is_none() {
                Self::create_chat_message(ctx, chat.tag(), strings::WELCOME_TEXT, None).await?;
            }
        }

        Ok(())
    }

    async fn create_chat_message(
        ctx: &Context,
        chat_tag: ChatTag,
        text: &str,
        reply_to_id: Option<u64>,
    ) -> Result<Publication<ChatMessage>> {
        let options = CreateChatMessageOptions {
            chat_tag,
            text: Some(text),
            reply_to_id,
            use_new_formatting: true,
            ..Default::default()
        };

        Publication::<ChatMessage>::create(&ctx.client, options)
            .await
            .map_err(Into::into)
    }
}
