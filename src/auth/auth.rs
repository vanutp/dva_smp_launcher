use colored::Colorize;

use super::base;
use super::base::AuthProvider;

use crate::lang;
use crate::config::runtime_config;

pub async fn auth_and_save(config: &mut runtime_config::Config) -> bool {
    let mut online = true;

    let mut tried_auth = false;
    for _ in 0..2 {
        let mut auth_provider = base::get_auth_provider();
        let token: String;
        if config.token.is_some() {
            token = config.token.clone().unwrap();
        } else {
            if tried_auth {
                panic!("{}", lang::get_loc(&config.lang).error_during_auth.red());
            }
            tried_auth = true;

            match auth_provider.authenticate().await {
                Ok(t) => token = t,
                Err(e) => {
                    if e.is_connect() {
                        online = false;
                        break;
                    } else {
                        panic!("{}", lang::get_loc(&config.lang).error_during_auth.red());
                    }
                }
            }
            config.token = Some(token.clone());
        }
    
        let user = auth_provider.get_user_info(&token).await;
        match user {
            Ok(user) => {
                config.user_info = Some(user);
                break;
            }
            Err(e) => {
                if e.is_connect() {
                    online = false;
                    break;
                } else if let Some(status) = e.status() {
                    if status.is_client_error() {
                        config.token = None;
                        continue;
                    }
                }
                panic!("{}: {:?}", lang::get_loc(&config.lang).error_during_user_info.red(), e);
            }
        }
    }

    if !online {
        if config.user_info.is_none() {
            panic!("{}", lang::get_loc(&config.lang).error_use_internet_for_first_connection.red());
        }
    }

    runtime_config::save_config(&config);

    return online;
}
