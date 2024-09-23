use std::{collections::HashMap, path::Path};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ObjectData {
    pub hash: String,
}

#[derive(Deserialize)]
pub struct AssetsMetadata {
    pub objects: HashMap<String, ObjectData>,
}

pub async fn read_asset_metadata(
    assets_metadata_path: &Path,
) -> Result<AssetsMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let data = tokio::fs::read(assets_metadata_path).await?;
    let data: AssetsMetadata = serde_json::from_slice(&data)?;
    Ok(data)
}
