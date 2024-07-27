mod config;
mod auth;
mod constants;
mod lang;
mod utils;
mod modpack;

use config::runtime_config;
use tokio;

#[tokio::main]
async fn main() {
    let mut config = runtime_config::load_config();

    utils::set_sigint_handler(&config);

    let online = auth::auth::auth_and_save(&mut config).await;
    
    println!("{} {}", online, config.user_info.as_ref().unwrap().username);
}
