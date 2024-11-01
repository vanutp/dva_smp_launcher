use crate::auth::microsoft::MicrosoftAuthProvider;
use crate::message_provider::MessageProvider;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::{utils::BoxResult, version::extra_version_metadata::AuthData};
use std::sync::Arc;

use super::{elyby::ElyByAuthProvider, none::NoneAuthProvider, telegram::TGAuthProvider};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct UserInfo {
    pub uuid: String,
    pub username: String,
}

#[async_trait]
pub trait AuthProvider {
    async fn authenticate(&self, message_provider: Arc<dyn MessageProvider>) -> BoxResult<String>;

    async fn get_user_info(&self, token: &str) -> BoxResult<UserInfo>;

    fn get_auth_url(&self) -> Option<String>;

    fn get_name(&self) -> String;
}

pub fn get_auth_provider(auth_data: &AuthData) -> Arc<dyn AuthProvider + Send + Sync> {
    match auth_data {
        AuthData::Microsoft(auth_data) => Arc::new(MicrosoftAuthProvider::new(&auth_data.client_id)),

        AuthData::ElyBy(auth_data) => Arc::new(ElyByAuthProvider::new(
            &auth_data.client_id,
            &auth_data.client_secret,
        )),

        AuthData::Telegram(auth_data) => Arc::new(TGAuthProvider::new(&auth_data.auth_base_url)),

        AuthData::None => Arc::new(NoneAuthProvider::new()),
    }
}
