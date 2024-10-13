#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auth;
mod config;
mod constants;
mod lang;
mod launcher;
mod message_provider;
mod update_app;
mod utils;
mod version;

use config::runtime_config::{self, setup_logger};
use utils::set_sigint_handler;

fn main() {
    set_sigint_handler();
    setup_logger();

    let config = runtime_config::load_config();
    update_app::app::run_gui(&config);
    app::app::run_gui(config);
}
