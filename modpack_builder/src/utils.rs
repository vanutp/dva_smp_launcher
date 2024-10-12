use std::{
    error::Error,
    path::{Path, PathBuf},
};

use log::info;
use shared::version::version_manifest::{fetch_version_manifest, VersionInfo};

#[derive(thiserror::Error, Debug)]
pub enum VanillaGeneratorError {
    #[error("Vanilla version not found")]
    VersionNotFound,
}

pub const VANILLA_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

pub async fn get_vanilla_version_info(
    minecraft_version: &str,
) -> Result<VersionInfo, Box<dyn Error + Send + Sync>> {
    let version_manifest = fetch_version_manifest(VANILLA_MANIFEST_URL).await?;
    let version_info = version_manifest
        .versions
        .iter()
        .find(|v| v.id == minecraft_version)
        .ok_or(VanillaGeneratorError::VersionNotFound)?;
    Ok(version_info.clone())
}

pub async fn exec_custom_command(command: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    exec_custom_command_in_dir(command, &Path::new(".")).await
}

pub async fn exec_custom_command_in_dir(
    command: &str,
    dir: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Executing command: {}", command);
    let mut cmd = tokio::process::Command::new("bash");
    cmd.args(vec!["-c", command]).current_dir(dir);
    let status = cmd.status().await?;
    if !status.success() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Command failed",
        )));
    }
    Ok(())
}

pub fn get_url_from_path(
    path: &Path,
    base_dir: &Path,
    download_server_base: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    Ok(format!(
        "{}/{}",
        download_server_base,
        path.strip_prefix(base_dir)?.to_string_lossy()
    ))
}

pub fn get_assets_dir(output_dir: &Path) -> PathBuf {
    let assets_dir = output_dir.join("assets");
    if !assets_dir.exists() {
        std::fs::create_dir_all(&assets_dir).unwrap();
    }
    assets_dir
}

pub fn to_abs_path_str(path: &Path) -> Result<String, Box<dyn Error + Send + Sync>> {
    Ok(path.canonicalize()?.to_string_lossy().to_string())
}
