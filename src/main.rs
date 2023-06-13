use std::thread;

use tokio::runtime;

mod ely_by;
mod config;

slint::include_modules!();

enum UiPage {
    Loading,
    Error,
    AuthWaiting,
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
                    ui.set_page(UiPage::Error as i32);
                    ui.set_error(error.into());
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

            let user_info = ely_by::get_user_info(config.token.unwrap().as_str()).await;
            if user_info.is_err() {
                set_error("Ошибка получения данных о пользователе");
                return;
            }
            let user_info = user_info.unwrap();


        })
    });

    ui.on_request_increase_value({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            ui.set_counter(ui.get_counter() + 0.1);
        }
    });

    ui.run().unwrap();
}
