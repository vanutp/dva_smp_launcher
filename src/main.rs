#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auth;
mod config;
mod constants;
mod files;
mod lang;
mod launcher;
mod message_provider;
mod modpack;
mod progress;
mod update_app;
mod utils;

use config::runtime_config;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_target(false)
        .format_timestamp(None)
        .init();

    utils::set_sigint_handler();

    let config = runtime_config::load_config();

    update_app::app::run_gui(&config);

    app::app::run_gui(config);
}
