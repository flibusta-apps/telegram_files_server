use once_cell::sync::Lazy;

fn get_env(env: &'static str) -> Result<String, String> {
    std::env::var(env).map_err(|_| format!("Cannot get the {env} env variable"))
}

fn get_non_empty_env(env: &'static str) -> Result<String, String> {
    let value = get_env(env)?;
    if value.trim().is_empty() {
        return Err(format!("{env} env variable must not be empty"));
    }
    Ok(value)
}

fn parse_env<T: std::str::FromStr>(env: &'static str) -> Result<T, String> {
    get_env(env)?
        .trim()
        .parse()
        .map_err(|_| format!("{env} env variable has an invalid value"))
}

/// Optional numeric env variable: default if unset, error if set but invalid or zero.
fn parse_env_or<T>(env: &'static str, default: T) -> Result<T, String>
where
    T: std::str::FromStr + PartialEq + Default,
{
    match std::env::var(env) {
        Err(_) => Ok(default),
        Ok(v) => {
            let parsed: T = v
                .trim()
                .parse()
                .map_err(|_| format!("{env} env variable has an invalid value"))?;
            if parsed == T::default() {
                return Err(format!("{env} env variable must be greater than zero"));
            }
            Ok(parsed)
        }
    }
}

pub struct Config {
    pub api_key: String,

    pub telegram_api_url: String,
    pub telegram_chat_id: i64,
    pub telegram_audio_chat_id: i64,
    pub telegram_temp_chat_id: i64,
    pub bot_tokens: Vec<String>,

    pub sentry_dsn: String,

    pub upload_timeout_secs: u64,
    pub max_upload_size: usize,
    pub max_concurrent_uploads: usize,
}

impl Config {
    pub fn try_load() -> Result<Config, String> {
        let api_key = get_non_empty_env("API_KEY")?;

        let telegram_api_url = get_non_empty_env("API_URL")?;
        reqwest::Url::parse(&telegram_api_url).map_err(|e| format!("Invalid API_URL: {e}"))?;

        let bot_tokens: Vec<String> = serde_json::from_str(&get_env("BOT_TOKENS")?)
            .map_err(|e| format!("BOT_TOKENS must be a JSON array of strings: {e}"))?;
        if bot_tokens.is_empty() || bot_tokens.iter().any(|t| t.trim().is_empty()) {
            return Err("BOT_TOKENS must contain at least one non-empty token".to_string());
        }

        Ok(Config {
            api_key,
            telegram_api_url,
            telegram_chat_id: parse_env("TELEGRAM_CHAT_ID")?,
            telegram_audio_chat_id: parse_env("TELEGRAM_AUDIO_CHAT_ID")?,
            telegram_temp_chat_id: parse_env("TELEGRAM_TEMP_CHAT_ID")?,
            bot_tokens,
            sentry_dsn: get_env("SENTRY_DSN")?,
            upload_timeout_secs: parse_env_or("UPLOAD_TIMEOUT_SECS", 600)?,
            max_upload_size: parse_env_or("MAX_UPLOAD_SIZE", 2usize * 1024 * 1024 * 1024)?,
            max_concurrent_uploads: parse_env_or("MAX_CONCURRENT_UPLOADS", 4)?,
        })
    }
}

pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    Config::try_load().unwrap_or_else(|err| {
        eprintln!("Configuration error: {err}");
        std::process::exit(1);
    })
});
