use anyhow::Context;

#[derive(Debug)]
pub struct ApplicationConfig {
    pub telegram: TelegramConfig,
}

#[derive(Debug)]
pub struct TelegramConfig {
    pub api_hash: String,
    pub api_id: i32,
}

impl ApplicationConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let telegram: TelegramConfig = TelegramConfig {
            api_hash: std::env::var("TELEGRAM_API_HASH").context("TELEGRAM_API_HASH is not set")?,
            api_id: std::env::var("TELEGRAM_API_ID")
                .context("TELEGRAM_API_ID is not set")?
                .parse::<i32>()
                .context("TELEGRAM_API_ID must be a valid integer")?,
        };

        Ok(ApplicationConfig { telegram })
    }
}
