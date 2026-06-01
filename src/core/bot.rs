use std::sync::Arc;

use once_cell::sync::Lazy;
use teloxide::Bot;

use crate::config::CONFIG;

pub struct RoundRobinBot {
    bots: Arc<Vec<Bot>>,
    current_index: std::sync::atomic::AtomicUsize,
}

impl RoundRobinBot {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(5 * 60))
            .tcp_nodelay(true)
            .build()
            .expect("Failed to build reqwest client");

        let api_url =
            reqwest::Url::parse(CONFIG.telegram_api_url.as_str()).expect("Invalid API_URL");

        let bots: Vec<Bot> = CONFIG
            .bot_tokens
            .iter()
            .map(|token| {
                Bot::with_client(token.clone(), client.clone()).set_api_url(api_url.clone())
            })
            .collect();

        RoundRobinBot {
            bots: Arc::new(bots),
            current_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn get_bot(&self) -> Bot {
        let index = self
            .current_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.bots.len();
        self.bots[index].clone()
    }
}

pub static ROUND_ROBIN_BOT: Lazy<RoundRobinBot> = Lazy::new(RoundRobinBot::new);
