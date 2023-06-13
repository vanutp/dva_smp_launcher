use std::path::Path;
use std::sync::Arc;
use std::thread;

use clone_macro::clone;
use tokio::runtime;
use tokio::sync::Notify;

mod ely_by;
mod config;
mod utils;

slint::include_modules!();

enum UiPage {
    Loading,
    Error,
    AuthWaiting,
    Config,
    Starting,
}

fn main() {
    let ui = AppWindow::new().unwrap();

    thread::spawn({
        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create asynchronous runtime");

        let ui_handle = ui.as_weak();
        move || rt.block_on(async {
            let set_page = |page: UiPage| {
                ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_page(page as i32);
                }).unwrap();
            };
            let set_error = |error: &str| {
                let error = error.to_string();
                ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_error(error.into());
                    ui.set_page(UiPage::Error as i32);
                }).unwrap();
            };
            let set_progress = |status: &str, progress: f32| {
                let status = status.to_string();
                ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_starting_status(status.into());
                    ui.set_starting_progress(progress);
                    ui.set_page(UiPage::Starting as i32);
                }).unwrap();
            };

            let mut config = config::load();

            if config.token.is_none() {
                set_page(UiPage::AuthWaiting);
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
                set_page(UiPage::Loading);
            }

            let user_info = ely_by::get_user_info(config.token.to_owned().unwrap().as_str()).await;
            if user_info.is_err() {
                set_error("Ошибка получения данных о пользователе");
                return;
            }
            let user_info = user_info.unwrap();

            let username = user_info.username;
            let xmx = config.xmx;
            drop(config);
            let notify = Arc::new(Notify::new());
            ui_handle.upgrade_in_event_loop(clone!([notify], move |ui| {
                ui.set_username(username.into());
                ui.set_memory(xmx.to_string().into());
                ui.set_page(UiPage::Config as i32);
                ui.on_start({
                    let ui_handle = ui.as_weak();
                    move || {
                        let ui = ui_handle.unwrap();
                        let mem = String::from(ui.get_memory().as_str());
                        let mem = str::parse::<i32>(mem.as_str());
                        if mem.is_err() {
                            ui.set_error("Ну блин в память надо число вводить, а теперь перезапускай лаунчер, потому что я не знаю раст((".into());
                            ui.set_page(UiPage::Error as i32);
                        } else {
                            let mut config = config::load();
                            config.xmx = mem.unwrap();
                            let config_save_res = config::save(&config);
                            if config_save_res.is_err() {
                                ui.set_error("Ошибка сохранения конфига".into());
                                ui.set_page(UiPage::Error as i32);
                                return;
                            }
                            notify.notify_one();
                        }
                    }
                })
            })).unwrap();

            notify.notified().await;

            let config = config::load();
            set_progress("Проверка и загрузка файлов сборки...", 0.5);
        })
    });

    ui.run().unwrap();
}
