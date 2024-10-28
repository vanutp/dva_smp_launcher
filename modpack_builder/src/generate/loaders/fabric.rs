use std::path::Path;

use async_trait::async_trait;
use log::info;
use reqwest::Client;
use serde::Deserialize;
use shared::{
    paths::get_versions_dir,
    utils::BoxResult,
    version::{version_manifest::VersionInfo, version_metadata::VersionMetadata},
};

use super::generator::{GeneratorResult, VersionGenerator};

const FABRIC_META_BASE_URL: &str = "https://meta.fabricmc.net/v2/versions/loader/";

#[derive(Deserialize)]
struct FabricVersionLoader {
    version: String,
}

#[derive(Deserialize)]
struct FabricVersionMeta {
    loader: FabricVersionLoader,
}

pub async fn get_latest_fabric_version(game_version: &str) -> BoxResult<String> {
    let fabric_manifest_url = format!("{}{}", FABRIC_META_BASE_URL, game_version);
    let client = Client::new();
    let response = client
        .get(&fabric_manifest_url)
        .send()
        .await?
        .error_for_status()?;
    let fabric_versions: Vec<FabricVersionMeta> = response.json().await?;
    return Ok(fabric_versions[0].loader.version.clone());
}

pub async fn download_fabric_metadata(
    minecraft_version: &str,
    loader_version: &str,
    output_dir: &Path,
) -> BoxResult<VersionMetadata> {
    let fabric_metadata_url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        minecraft_version, loader_version
    );
    let version_metadata = VersionMetadata::fetch(&fabric_metadata_url).await?;
    let versions_dir = get_versions_dir(&output_dir);
    version_metadata.save(&versions_dir).await?;
    Ok(version_metadata)
}

pub struct FabricGenerator {
    version_name: String,
    vanilla_version_info: VersionInfo,
    loader_version: Option<String>,
}

impl FabricGenerator {
    pub fn new(
        version_name: String,
        vanilla_version_info: VersionInfo,
        loader_version: Option<String>,
    ) -> Self {
        Self {
            version_name,
            vanilla_version_info,
            loader_version,
        }
    }
}

#[async_trait]
impl VersionGenerator for FabricGenerator {
    async fn generate(&self, work_dir: &Path) -> BoxResult<GeneratorResult> {
        let minecraft_version = self.vanilla_version_info.id.clone();

        info!(
            "Generating Fabric modpack \"{}\", minecraft version {}",
            self.version_name, minecraft_version
        );

        info!("Downloading vanilla version metadata");
        let vanilla_metadata = VersionMetadata::read_or_download(
            &self.vanilla_version_info.get_parent_metadata_info(),
            &get_versions_dir(work_dir),
        )
        .await?;

        let fabric_version = match &self.loader_version {
            Some(loader_version) => loader_version.clone(),
            None => {
                let version = get_latest_fabric_version(&minecraft_version).await?;
                info!(
                    "Loader version not specified, using latest version: {}",
                    version
                );
                version
            }
        };

        info!("Downloading Fabric version metadata");
        let fabric_metadata =
            download_fabric_metadata(&minecraft_version, &fabric_version, &work_dir).await?;

        info!("Fabric version \"{}\" generated", self.version_name);

        Ok(GeneratorResult {
            metadata: vec![vanilla_metadata, fabric_metadata],
            extra_libs_paths: vec![],
        })
    }
}
