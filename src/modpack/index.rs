use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::config::build_config;

#[derive(Clone, Serialize, Deserialize)]
pub struct Object {
    pub path: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ModpackIndex {
    pub modpack_name: String,
    pub modpack_version: String,
    pub include: Vec<String>,
    pub include_no_overwrite: Vec<String>,
    pub objects: Vec<Object>,
    pub resources_url_base: Option<String>,
    pub java_version: String,
}

const DEFAULT_RESOURCES_URL_BASE: &str = "https://resources.download.minecraft.net";

impl ModpackIndex {
    pub fn get_resources_url_base(&self) -> &str {
        self.resources_url_base
            .as_ref()
            .map(|x| x.as_str())
            .unwrap_or(DEFAULT_RESOURCES_URL_BASE)
    }
}

#[derive(Serialize, Deserialize)]
struct ModpackIndexes {
    modpack_indexes: Vec<ModpackIndex>,
}

pub async fn load_remote_indexes(
) -> Result<Vec<ModpackIndex>, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let res = client
        .get(format!("{}/index.json", build_config::get_modpacks_base()))
        .send()
        .await?
        .error_for_status()?
        .json::<ModpackIndexes>()
        .await?;
    Ok(res.modpack_indexes)
}

pub fn load_local_indexes(index_path: &Path) -> Vec<ModpackIndex> {
    if !index_path.is_file() {
        return vec![];
    }
    match fs::read_to_string(index_path) {
        Ok(data) => match serde_json::from_str::<ModpackIndexes>(&data) {
            Ok(indexes) => indexes.modpack_indexes,
            Err(_) => vec![],
        },
        Err(_) => vec![],
    }
}

pub fn save_local_index(index_path: &Path, index: &ModpackIndex) {
    let mut indexes = load_local_indexes(&index_path);
    indexes.retain(|x| x.modpack_name != index.modpack_name);
    let mut indexes: Vec<&ModpackIndex> = indexes.iter().collect();
    indexes.push(index);
    if let Ok(data) = serde_json::to_string_pretty(&indexes) {
        let _ = fs::write(&index_path, data);
    }
}
