use crate::lang::LangMessage;
use crate::message_provider::MessageProvider;

use super::base::{AuthProvider, UserInfo};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use std::sync::Arc;
use std::{collections::HashMap, time::Duration};

#[derive(Deserialize)]
struct LoginStartResponse {
    code: String,
    intermediate_token: String,
}

#[derive(Deserialize)]
struct BotInfo {
    bot_username: String,
}

pub struct TGAuthProvider {
    client: Client,
    base_url: String,
}

impl TGAuthProvider {
    pub fn new(base_url: &str) -> Self {
        TGAuthProvider {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    async fn get_bot_name(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let body = self
            .client
            .get(format!("{}/info", self.base_url))
            .send()
            .await?
            .text()
            .await?;

        let bot_info: BotInfo = serde_json::from_str(&body)?;
        Ok(bot_info.bot_username)
    }
}

#[async_trait]
impl AuthProvider for TGAuthProvider {
    async fn authenticate(
        &self,
        message_provider: Arc<dyn MessageProvider>,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let bot_name = self.get_bot_name().await?;
        let body = self
            .client
            .post(format!("{}/login/start", self.base_url))
            .send()
            .await?
            .text()
            .await?;
        let start_resp: LoginStartResponse = serde_json::from_str(&body)?;

        let tg_deeplink = format!("https://t.me/{}?start={}", bot_name, start_resp.code);
        let _ = open::that(&tg_deeplink);
        message_provider.set_message(LangMessage::AuthMessage { url: tg_deeplink });

        let access_token;
        loop {
            let response = self
                .client
                .post(format!("{}/login/poll", self.base_url))
                .json(&serde_json::json!({
                    "intermediate_token": start_resp.intermediate_token
                }))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    resp.error_for_status_ref()?;

                    let body = resp.text().await?;
                    let poll_resp: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&body)?;

                    access_token = poll_resp
                        .get("user")
                        .unwrap()
                        .get("access_token")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string();
                    break;
                }
                Err(e) => {
                    if !e.is_timeout() {
                        return Err(Box::new(e));
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(1));
        }

        Ok(access_token)
    }

    async fn get_user_info(&self, token: &str) -> Result<UserInfo, Box<dyn Error + Send + Sync>> {
        let resp = self
            .client
            .get(format!("{}/login/profile", self.base_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        resp.error_for_status_ref()?;

        let body = resp.text().await?;
        let user_info: UserInfo = serde_json::from_str(&body).unwrap();
        Ok(user_info)
    }

    fn get_auth_url(&self) -> Option<String> {
        Some(self.base_url.clone())
    }

    fn get_name(&self) -> String {
        "Telegram".to_string()
    }
}
