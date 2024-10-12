use std::{error::Error, path::Path};

use async_trait::async_trait;
use log::info;
use shared::{
    paths::get_versions_dir,
    version::{
        version_manifest::VersionInfo,
        version_metadata::{fetch_version_metadata, save_version_metadata, VersionMetadata},
    },
};

use crate::{
    generate::{patch::replace_download_urls, sync::sync_version},
    utils::get_vanilla_version_info,
};

use super::generator::{GeneratorResult, VersionGenerator};

pub struct VanillaGenerator {
    version_name: String,
    minecraft_version: String,
    download_server_base: String,
    replace_download_urls: bool,
}

impl VanillaGenerator {
    pub fn new(
        version_name: String,
        minecraft_version: String,
        download_server_base: String,
        replace_download_urls: bool,
    ) -> Self {
        Self {
            version_name,
            minecraft_version,
            download_server_base,
            replace_download_urls,
        }
    }
}

pub async fn download_vanilla_metadata(
    version_info: &VersionInfo,
    output_dir: &Path,
) -> Result<VersionMetadata, Box<dyn Error + Send + Sync>> {
    let version_metadata = fetch_version_metadata(version_info).await?;
    let versions_dir = get_versions_dir(&output_dir);
    save_version_metadata(&versions_dir, &version_metadata).await?;
    return Ok(version_metadata);
}

#[async_trait]
impl VersionGenerator for VanillaGenerator {
    async fn generate(
        &self,
        output_dir: &Path,
        _: &Path,
    ) -> Result<GeneratorResult, Box<dyn Error + Send + Sync>> {
        info!(
            "Generating vanilla version \"{}\", minecraft version {}",
            self.version_name, self.minecraft_version
        );

        info!("Fetching version manifest");
        let version_info = get_vanilla_version_info(&self.minecraft_version).await?;

        info!("Downloading version metadata");
        let mut vanilla_metadata = download_vanilla_metadata(&version_info, output_dir).await?;

        if self.replace_download_urls {
            info!("Syncing version");
            sync_version(&vanilla_metadata, &self.version_name, output_dir).await?;

            replace_download_urls(
                &self.version_name,
                &mut vanilla_metadata,
                &self.download_server_base,
                output_dir,
            )
            .await?;

            let versions_dir = get_versions_dir(&output_dir);
            save_version_metadata(&versions_dir, &vanilla_metadata).await?;
        }

        info!("Vanilla version \"{}\" generated", self.version_name);

        Ok(GeneratorResult {
            id: vanilla_metadata.id.clone(),
            extra_libs_paths: vec![],
        })
    }
}
