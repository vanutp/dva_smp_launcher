use std::{error::Error, path::{Path, PathBuf}};

use log::info;

pub async fn exec_custom_command(command: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Executing command: {}", command);
    let mut command = command.split_whitespace();
    let command_name = command.next().unwrap();
    let mut cmd = tokio::process::Command::new(command_name);
    cmd.args(command);
    let status = cmd.status().await?;
    if !status.success() {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Command failed")));
    }
    Ok(())
}

pub fn get_url_from_path(path: &Path, base_dir: &Path, download_server_base: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    Ok(format!(
        "{}/{}",
        download_server_base,
        path.strip_prefix(base_dir)?.to_string_lossy()
    ))
}

pub fn get_assets_dir(output_dir: &Path) -> PathBuf {
    output_dir.join("assets")
}
