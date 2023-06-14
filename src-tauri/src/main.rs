// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::Duration;
use clone_macro::clone;
use futures::SinkExt;
use tauri::{LogicalSize, Manager};
use tauri::Size::Logical;
use tokio::sync::{Notify, oneshot};
use serde::{Serialize, Deserialize};
use tokio::time::sleep;
use crate::utils::sync_modpack;

mod ely_by;
mod config;
mod utils;

#[derive(Clone, Serialize)]
struct SetError {
    message: String,
}

#[derive(Clone, Serialize)]
struct SetProgress {
    message: String,
    progress: Option<f32>,
}

enum UiPage {
    Loading,
    Error,
    Config,
}

#[derive(Clone, Serialize)]
struct SetConfig {
    username: String,
    memory: i32,
    java_path: String,
}

#[derive(Clone, Deserialize, Debug)]
struct StartRequest {
    memory: i32,
    java_path: String,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            tauri::async_runtime::spawn(async move {
                sleep(Duration::from_secs(1)).await;
                t
                let app = app_handle.app_handle();
                let app2 = app_handle.app_handle();
                let set_progress = move |message: &str, progress: Option<f32>| {
                    app2.emit_all("set_progress", SetProgress {
                        message: message.to_string(),
                        progress,
                    }).unwrap();
                };
                let set_error = |message: &str| {
                    app.emit_all("set_error", SetError {
                        message: message.to_string(),
                    }).unwrap();
                };
                let set_config = |username: String, memory: i32, java_path: String| {
                    app.emit_all("set_config", SetConfig {
                        username,
                        memory,
                        java_path,
                    }).unwrap();
                };

                let mut config = config::load();

                if config.token.is_none() {
                    set_progress("Авторизуйтесь в открывшемся окне браузера...", None);
                    let token = ely_by::authorize().await;
                    if token.is_err() {
                        set_error("Ошибка получения токена");
                        return;
                    }
                    config.token = Some(token.unwrap());
                    let config_save_res = config::save(&config);
                    if config_save_res.is_err() {
                        set_error("Ошибка сохранения конфига");
                        return;
                    }
                    set_progress("Загрузка...", None);
                }

                let user_info = ely_by::get_user_info(config.token.to_owned().unwrap().as_str()).await;
                if user_info.is_err() {
                    set_error("Ошибка получения данных о пользователе");
                    return;
                }
                let user_info = user_info.unwrap();

                set_config(user_info.username, config.xmx, config.java_path.unwrap_or("".to_string()));

                let (tx, rx) = oneshot::channel();
                let tx = Mutex::new(Some(tx));
                app.listen_global("start", move |event| {
                    let payload = serde_json::from_str::<StartRequest>(event.payload().unwrap()).unwrap();
                    tx.lock().unwrap().take().unwrap().send(payload).unwrap();
                });
                let payload = rx.await.unwrap();
                config.xmx = payload.memory;
                config.java_path = payload.java_path.into();
                let config_save_res = config::save(&config);
                if config_save_res.is_err() {
                    set_error("Ошибка сохранения конфига".into());
                    return;
                }

                sync_modpack(move |status: &str, progress: f32| {
                    set_progress(status, progress.into());
                }).await.unwrap();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
