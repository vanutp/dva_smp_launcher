use crate::message_provider::MessageProvider;

use super::base::{AuthProvider, UserInfo};
use async_trait::async_trait;
use shared::utils::BoxResult;
use std::sync::Arc;

pub struct NoneAuthProvider {}

impl NoneAuthProvider {
    pub fn new() -> Self {
        NoneAuthProvider {}
    }
}

#[async_trait]
impl AuthProvider for NoneAuthProvider {
    async fn authenticate(&self, _: Arc<dyn MessageProvider>) -> BoxResult<String> {
        Ok("".to_string())
    }

    async fn get_user_info(&self, _: &str) -> BoxResult<UserInfo> {
        Ok(UserInfo {
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            username: "demo".to_string(),
        })
    }

    fn get_auth_url(&self) -> Option<String> {
        None
    }

    fn get_name(&self) -> String {
        "No auth provider".to_string()
    }
}
