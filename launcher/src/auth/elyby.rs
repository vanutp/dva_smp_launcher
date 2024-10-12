use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::sync::mpsc::channel;
use std::{
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc::unbounded_channel, oneshot};
use warp::Filter;
use warp::{http::Uri, reply::Reply};

use crate::lang::LangMessage;
use crate::message_provider::MessageProvider;

use super::base::{AuthProvider, UserInfo};

const ELY_BY_BASE: &str = "https://ely.by/";

// TODO: rewrite this horrible code
#[derive(Debug)]
struct InvalidCodeError;

impl std::error::Error for InvalidCodeError {}

impl std::fmt::Display for InvalidCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid code error")
    }
}

pub struct ElyByAuthProvider {
    app_name: String,
    client_id: String,
    client_secret: String,
}

#[derive(Deserialize)]
struct AuthQuery {
    code: String,
}

impl ElyByAuthProvider {
    pub fn new(elyby_app_name: &str, elyby_client_id: &str, elyby_client_secret: &str) -> Self {
        ElyByAuthProvider {
            app_name: elyby_app_name.to_string(),
            client_id: elyby_client_id.to_string(),
            client_secret: elyby_client_secret.to_string(),
        }
    }

    fn print_auth_url(&self, redirect_uri: &str, message_provider: Arc<dyn MessageProvider>) {
        let url = format!(
            "https://account.ely.by/oauth2/v1?client_id={}&redirect_uri={}&response_type=code&scope=account_info%20minecraft_server_session&prompt=select_account",
            &self.client_id, redirect_uri
        );
        let _ = open::that(&url);
        message_provider.set_message(LangMessage::AuthMessage { url });
    }

    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = Client::new();
        let resp = client
            .post("https://account.ely.by/api/oauth2/v1/token")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
                ("code", code),
            ])
            .send()
            .await?;

        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;
        if status != 200 {
            if data["error"] == "invalid_request" {
                return Err(Box::new(InvalidCodeError));
            }
        }

        if data["token_type"] != "Bearer" {
            return Err("Invalid token type".into());
        }

        Ok(data["access_token"].as_str().unwrap().to_string())
    }
}

#[async_trait]
impl AuthProvider for ElyByAuthProvider {
    async fn authenticate(
        &self,
        message_provider: Arc<dyn MessageProvider>,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let (code_tx, mut code_rx) = unbounded_channel();
        let code_tx = Arc::new(Mutex::new(code_tx));

        let (error_tx, error_rx) = channel::<()>();
        let error_rx = Arc::new(Mutex::new(error_rx));

        let handle = {
            let code_tx = Arc::clone(&code_tx);
            let app_name = self.app_name.clone();
            warp::any()
                .and(warp::query::<AuthQuery>())
                .map(move |query: AuthQuery| {
                    let code_tx = code_tx.lock().unwrap();
                    code_tx.send(query.code.clone()).unwrap();

                    let error_rx = error_rx.lock().unwrap();
                    let error = error_rx.recv();
                    if error.is_ok() {
                        Box::new(warp::http::StatusCode::INTERNAL_SERVER_ERROR) as Box<dyn Reply>
                    } else {
                        Box::new(warp::redirect::temporary(
                            Uri::from_maybe_shared(format!(
                                "https://account.ely.by/oauth2/code/success?appName={}",
                                &app_name
                            ))
                            .unwrap(),
                        ))
                    }
                })
        };

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (addr, server) =
            warp::serve(handle).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                shutdown_rx.await.ok();
            });

        let redirect_uri = format!("http://localhost:{}/", addr.port());

        tokio::spawn(server);
        self.print_auth_url(&redirect_uri, message_provider);

        loop {
            let code = code_rx.recv().await.unwrap();
            match self.exchange_code(&code, &redirect_uri).await {
                Ok(token) => {
                    shutdown_tx.send(()).unwrap();
                    return Ok(token);
                }
                Err(e) => {
                    if e.downcast_ref::<InvalidCodeError>().is_none() {
                        shutdown_tx.send(()).unwrap();
                        return Err(e);
                    }
                    error_tx.send(()).unwrap();
                }
            }
        }
    }

    async fn get_user_info(&self, token: &str) -> Result<UserInfo, Box<dyn Error + Send + Sync>> {
        let client = Client::new();
        let resp = client
            .get("https://account.ely.by/api/account/v1/info")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        let data: UserInfo = resp.json().await?;
        Ok(data)
    }

    fn get_auth_url(&self) -> Option<String> {
        Some(ELY_BY_BASE.to_string())
    }

    fn get_name(&self) -> String {
        "Ely.by".to_string()
    }
}
