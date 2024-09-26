use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ObjectData {
    pub hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct AssetsMetadata {
    pub objects: HashMap<String, ObjectData>,
}

pub async fn fetch_asset_metadata(
    client: &reqwest::Client,
    url: &str,
) -> Result<AssetsMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url).send().await?.json().await?;
    Ok(response)
}

async fn get_assets_metadata_path(
    assets_dir: &Path,
    asset_id: &str,
) -> Result<PathBuf, std::io::Error> {
    tokio::fs::create_dir_all(assets_dir.join("indexes")).await?;
    Ok(assets_dir
        .join("indexes")
        .join(format!("{}.json", asset_id)))
}

pub async fn read_asset_metadata(
    asset_id: &str,
    assets_dir: &Path,
) -> Result<AssetsMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let data = tokio::fs::read(get_assets_metadata_path(&assets_dir, asset_id).await?).await?;
    let data: AssetsMetadata = serde_json::from_slice(&data)?;
    Ok(data)
}

pub async fn save_asset_metadata(
    asset_id: &str,
    metadata: &AssetsMetadata,
    assets_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = serde_json::to_vec(metadata)?;
    tokio::fs::write(get_assets_metadata_path(&assets_dir, asset_id).await?, data).await?;
    Ok(())
}
