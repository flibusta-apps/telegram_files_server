use once_cell::sync::Lazy;


fn get_env(env: &'static str) -> String {
    std::env::var(env).unwrap_or_else(|_| panic!("Cannot get the {} env variable", env))
}

pub struct Config {
    pub api_key: String,

    pub telegram_api_url: String,
    pub telegram_chat_id: i64,
    pub telegram_temp_chat_id: i64,
    pub bot_tokens: Vec<String>,

    pub sentry_dsn: String,
}


impl Config {
    pub fn load() -> Config {
        Config {
            api_key: get_env("API_KEY"),

            telegram_api_url: get_env("API_URL"),
            telegram_chat_id: get_env("TELEGRAM_CHAT_ID").parse().unwrap(),
            telegram_temp_chat_id: get_env("TELEGRAM_TEMP_CHAT_ID").parse().unwrap(),
            
            bot_tokens: serde_json::from_str(&get_env("BOT_TOKENS")).unwrap(),
            sentry_dsn: get_env("SENTRY_DSN"),
        }
    }
}

pub static CONFIG: Lazy<Config> = Lazy::new(Config::load);
