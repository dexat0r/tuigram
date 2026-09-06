use std::io::Write;

use tracing_subscriber::EnvFilter;

use crate::config::ApplicationConfig;

mod config;
mod telegram;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    setup_logger();

    let app_config: ApplicationConfig = ApplicationConfig::from_env()?;

    tracing::debug!(
        api_id = app_config.telegram.api_id,
        "Application config loaded"
    );

    let mut client = telegram::TelegramClient::new(&app_config.telegram).await?;

    if !client.is_authorized().await? {
        let phone = prompt("Phone number: ")?;
        client.request_login_code(&phone).await?;

        let login_result = loop {
            let code = prompt("Telegram code: ")?;

            match client.sign_in(&code).await? {
                telegram::LoginResult::CodeInvalid => eprintln!("Incorrect code, try again."),
                result => break result,
            }
        };

        match login_result {
            telegram::LoginResult::Authorized => {}
            telegram::LoginResult::PasswordRequired => loop {
                let password = rpassword::prompt_password("2FA password: ")?;
                match client.check_password(&password).await? {
                    telegram::LoginResult::Authorized => break,
                    telegram::LoginResult::InvalidPassword => {
                        eprintln!("Password invalid. Try again.")
                    }
                    _ => unreachable!(),
                }
            },
            telegram::LoginResult::SignUpRequired => anyhow::bail!("Sign up required"),
            telegram::LoginResult::CodeInvalid => unreachable!(),
            telegram::LoginResult::InvalidPassword => unreachable!(),
        }
    }

    client.shutdown().await;

    Ok(())
}

fn prompt(msg: &str) -> anyhow::Result<String> {
    print!("{msg}");
    std::io::stdout().flush()?;

    let mut value = String::new();
    std::io::stdin().read_line(&mut value)?;

    Ok(value.trim().to_owned())
}

fn setup_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init()
}
