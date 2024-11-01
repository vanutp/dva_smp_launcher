use std::sync::Arc;
use std::sync::Mutex;

use shared::utils::BoxResult;

use crate::config::runtime_config::VersionAuthData;
use crate::lang::LangMessage;
use crate::message_provider::MessageProvider;

use super::base::AuthProvider;
use super::base::UserInfo;

struct AuthMessageState {
    auth_message: Option<LangMessage>,
}

pub struct AuthMessageProvider {
    state: Arc<Mutex<AuthMessageState>>,
    ctx: egui::Context,
}

impl AuthMessageProvider {
    pub fn new(ctx: &egui::Context) -> Self {
        Self {
            state: Arc::new(Mutex::new(AuthMessageState { auth_message: None })),
            ctx: ctx.clone(),
        }
    }
}

impl MessageProvider for AuthMessageProvider {
    fn set_message(&self, message: LangMessage) {
        if let LangMessage::AuthMessage { .. } = message {
            let mut state = self.state.lock().unwrap();
            state.auth_message = Some(message);
            self.ctx.request_repaint();
        } else {
            panic!("Expected AuthMessage, got {:?}", message);
        }
    }

    fn get_message(&self) -> Option<LangMessage> {
        let state = self.state.lock().unwrap();
        return state.auth_message.clone();
    }

    fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.auth_message = None;
        self.ctx.request_repaint();
    }
}

pub async fn auth(
    existing_token: Option<String>,
    auth_provider: Arc<dyn AuthProvider + Send + Sync>,
    auth_message_provider: Arc<AuthMessageProvider>,
) -> BoxResult<VersionAuthData> {
    let mut token = existing_token;
    let mut user_info: Option<UserInfo> = None;

    let tries_num = if token.is_some() { 2 } else { 1 };
    for i in 0..tries_num {
        if token.is_none() {
            token = Some(
                auth_provider
                    .authenticate(auth_message_provider.clone())
                    .await?,
            );
        }

        match auth_provider.get_user_info(token.as_ref().unwrap()).await {
            Ok(info) => {
                user_info = Some(info);
                break;
            }

            Err(e) => {
                println!("Error: {:?}", e);
                let mut token_error = false;
                if let Some(re) = e.downcast_ref::<reqwest::Error>() {
                    if let Some(status) = re.status() {
                        if status.is_client_error() {
                            token_error = true;
                        }
                    }
                }

                if token_error && i + 1 != tries_num {
                    // try again with a new token in the next iteration
                    token = None;
                } else {
                    return Err(e);
                }
            }
        }
    }

    return Ok(VersionAuthData {
        token: token.unwrap(),
        user_info: user_info.unwrap(),
    });
}
