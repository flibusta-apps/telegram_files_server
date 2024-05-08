use std::{sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
}, time::Duration};

use once_cell::sync::Lazy;
use teloxide::Bot;

use crate::config::{self, CONFIG};

pub struct RoundRobinBot {
    bot_tokens: Arc<Vec<String>>,
    current_index: AtomicUsize,
}

impl RoundRobinBot {
    pub fn new(bot_tokens: Vec<String>) -> Self {
        RoundRobinBot {
            bot_tokens: Arc::new(bot_tokens),
            current_index: AtomicUsize::new(0),
        }
    }

    pub fn get_bot(&self) -> Bot {
        let index = self.current_index.fetch_add(1, Ordering::Relaxed) % self.bot_tokens.len();

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(5 * 60))
            .tcp_nodelay(true)
            .build()
            .unwrap();

        Bot::with_client(
            self.bot_tokens[index].clone(),
            client
        ).set_api_url(reqwest::Url::parse(CONFIG.telegram_api_url.as_str()).unwrap())
    }
}

pub static ROUND_ROBIN_BOT: Lazy<RoundRobinBot> =
    Lazy::new(|| RoundRobinBot::new(config::CONFIG.bot_tokens.clone()));
