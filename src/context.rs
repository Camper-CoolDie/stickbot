use std::collections::VecDeque;
use std::sync::Arc;

use bonfire::Client;
use bonfire::fcm::Client as FcmClient;
use bonfire::models::{Account, Config as BonfireConfig, Settings};
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::Config;

#[derive(Clone)]
pub struct Context {
    pub config: Arc<Config>,

    pub client: Client,
    pub fcm_client: FcmClient,
    pub b_config: Arc<BonfireConfig>,
    pub account: Arc<Account>,
    pub settings: Arc<Settings>,

    pub pool: SqlitePool,
    pub cancellation_token: CancellationToken,

    pub chat_cache: Arc<Mutex<VecDeque<u64>>>,
}
