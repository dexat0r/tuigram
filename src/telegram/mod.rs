use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use grammers_client::{
    Client, SenderPool, SignInError,
    client::{LoginToken, PasswordToken},
};
use grammers_session::updates::UpdatesLike;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};

use crate::config::TelegramConfig;

const SESSION_FILE: &str = "telegram-tui.session";

pub enum LoginResult {
    Authorized,
    CodeInvalid,
    PasswordRequired,
    SignUpRequired,
    InvalidPassword,
}

pub struct TelegramClient {
    client: grammers_client::Client,
    runner_handle: grammers_client::sender::SenderPoolHandle,
    runner_task: JoinHandle<()>,
    updates: Option<UnboundedReceiver<UpdatesLike>>,
    config: TelegramConfig,

    pending_login: Option<LoginToken>,
    pending_password: Option<PasswordToken>,
}

impl TelegramClient {
    pub async fn new(config: &TelegramConfig) -> anyhow::Result<Self> {
        let state_path = Self::resolve_session_folder()?;
        let session = Arc::new(
            grammers_client::session::storages::SqliteSession::open(state_path.join(SESSION_FILE))
                .await?,
        );
        let SenderPool {
            runner,
            handle,
            updates,
        } = SenderPool::new(Arc::clone(&session), config.api_id);

        let runner_handle = handle.thin.clone();
        let client = Client::new(handle);
        let runner_task = tokio::spawn(runner.run());

        Ok(Self {
            client,
            runner_handle,
            runner_task,
            updates: Some(updates),
            config: config.clone(),
            pending_login: None,
            pending_password: None,
        })
    }

    pub async fn is_authorized(&self) -> anyhow::Result<bool> {
        Ok(self.client.is_authorized().await?)
    }

    pub async fn request_login_code(&mut self, phone: &str) -> anyhow::Result<()> {
        let token = self
            .client
            .request_login_code(phone, &self.config.api_hash)
            .await?;

        self.pending_login = Some(token);

        Ok(())
    }

    pub async fn sign_in(&mut self, code: &str) -> anyhow::Result<LoginResult> {
        let login_token = self
            .pending_login
            .take()
            .context("start login flow with requesting code")?;

        Ok(match self.client.sign_in(&login_token, code).await {
            Ok(_) => LoginResult::Authorized,
            Err(SignInError::SignUpRequired) => LoginResult::SignUpRequired,
            Err(SignInError::PasswordRequired(token)) => {
                self.pending_password = Some(token);
                LoginResult::PasswordRequired
            }
            Err(SignInError::InvalidCode) => {
                self.pending_login = Some(login_token);
                LoginResult::CodeInvalid
            }
            Err(SignInError::InvalidPassword(_)) => {
                anyhow::bail!("Telegram returned an invalid password error during code sign-in")
            }
            Err(SignInError::Other(error)) => {
                return Err(error).context("failed to login to Telegram");
            }
        })
    }

    pub async fn check_password(&mut self, password: &str) -> anyhow::Result<LoginResult> {
        let password_token = self
            .pending_password
            .take()
            .context("can not verify password because Telegram never asks for password")?;

        match self.client.check_password(password_token, password).await {
            Ok(_) => Ok(LoginResult::Authorized),
            Err(SignInError::InvalidPassword(token)) => {
                self.pending_password = Some(token);
                Ok(LoginResult::InvalidPassword)
            }
            Err(SignInError::Other(error)) => {
                Err(error).context("failed to verify Telegram 2FA password")
            }
            Err(error) => {
                anyhow::bail!("unexpected Telegram response during 2FA verification: {error}")
            }
        }
    }

    pub async fn shutdown(self) -> () {
        self.runner_handle.quit();
        let _ = self.runner_task.await;
    }

    fn resolve_session_folder() -> anyhow::Result<PathBuf> {
        let state_folder = match std::env::var_os("XDG_STATE_HOME") {
            Some(path) if !path.is_empty() => Path::new(&path)
                .is_absolute()
                .then(|| PathBuf::from(path))
                .context("$XDG_STATE_HOME must be absolute")?,
            _ => {
                let home = std::env::home_dir()
                    .context("could not determine home directory; set $HOME or $XDG_STATE_HOME")?;
                home.join(".local").join("state")
            }
        };

        let state_dir = state_folder.join("telegram-tui");

        std::fs::create_dir_all(&state_dir)
            .context("failed to create application state directory")?;

        Ok(state_dir)
    }
}
