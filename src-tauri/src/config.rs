use std::fs;
use std::fs::create_dir_all;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub token: Option<String>,
    pub java_path: Option<String>,
    pub xmx: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: None,
            java_path: None,
            xmx: 3072,
        }
    }
}

fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("dev", "vanutp", "DVA SMP").unwrap()
}

fn get_config_path() -> PathBuf {
    let dirs = get_dirs();
    if !dirs.config_dir().exists() {
        create_dir_all(dirs.config_dir()).unwrap();
    }
    dirs.config_dir().join("config.json")
}

pub fn get_minecraft_dir() -> PathBuf {
    let dirs = get_dirs();
    let minecraft_dir = dirs.data_dir().join(".minecraft");
    if !minecraft_dir.exists() {
        create_dir_all(minecraft_dir.clone()).unwrap();
    }
    minecraft_dir
}

pub fn load() -> Config {
    let config_path = get_config_path();

    config_path.exists()
        .then_some(config_path)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) -> anyhow::Result<()> {
    let config_path = get_config_path();
    fs::write(
        config_path,
        serde_json::to_string_pretty(config)?,
    )?;
    Ok(())
}
