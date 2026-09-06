use crate::config::ApplicationConfig;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    setup_logger();

    let app_config: ApplicationConfig = ApplicationConfig::from_env()?;

    tracing::debug!(
        api_id = app_config.telegram.api_id,
        "Application config loaded"
    );

    Ok(())
}

fn setup_logger() {
    tracing_subscriber::fmt().json().init()
}
