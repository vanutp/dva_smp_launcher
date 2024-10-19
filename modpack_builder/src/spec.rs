use log::{error, info, warn};
use shared::{
    paths::get_manifest_path,
    version::{
        extra_version_metadata::AuthData,
        version_manifest::{save_local_version_manifest, VersionManifest},
    },
};
use std::{error::Error, path::Path};
use tokio::fs;

use serde::Deserialize;

use crate::{
    generate::{
        extra::ExtraMetadataGenerator,
        loaders::{
            fabric::FabricGenerator,
            forge::{ForgeGenerator, Loader},
            generator::VersionGenerator,
            vanilla::VanillaGenerator,
        },
        manifest::get_version_info,
    },
    utils::exec_custom_command,
};

fn vanilla() -> String {
    "vanilla".to_string()
}

#[derive(Deserialize)]
pub struct Version {
    pub name: String,
    pub minecraft_version: String,

    #[serde(default = "vanilla")]
    pub loader_name: String,

    pub loader_version: Option<String>,

    #[serde(default)]
    pub include: Vec<String>,

    #[serde(default)]
    pub include_no_overwrite: Vec<String>,

    pub include_from: Option<String>,

    #[serde(default)]
    pub replace_download_urls: bool,

    #[serde(default)]
    pub auth_provider: AuthData,

    pub exec_before: Option<String>,
    pub exec_after: Option<String>,
}

#[derive(Deserialize)]
pub struct VersionsSpec {
    pub download_server_base: String,
    pub resources_url_base: Option<String>,
    pub versions: Vec<Version>,
    pub exec_before_all: Option<String>,
    pub exec_after_all: Option<String>,
}

impl VersionsSpec {
    pub async fn from_file(path: &Path) -> Result<VersionsSpec, Box<dyn Error + Send + Sync>> {
        let content = fs::read_to_string(path).await?;
        let spec = serde_json::from_str(&content)?;
        Ok(spec)
    }

    pub async fn generate(
        &self,
        output_dir: &Path,
        work_dir: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(command) = &self.exec_before_all {
            exec_custom_command(&command).await?;
        }

        let mut version_manifest = VersionManifest { versions: vec![] };

        for version in &self.versions {
            if let Some(command) = &version.exec_before {
                exec_custom_command(&command).await?;
            }

            let generator: Box<dyn VersionGenerator>;
            match version.loader_name.as_str() {
                "vanilla" => {
                    if version.loader_version.is_some() {
                        warn!("Ignoring loader version for vanilla version");
                    }

                    generator = Box::new(VanillaGenerator::new(
                        version.name.clone(),
                        version.minecraft_version.clone(),
                        self.download_server_base.clone(),
                        version.replace_download_urls,
                    ));
                }

                "fabric" => {
                    generator = Box::new(FabricGenerator::new(
                        version.name.clone(),
                        version.minecraft_version.clone(),
                        version.loader_version.clone(),
                        self.download_server_base.clone(),
                        version.replace_download_urls,
                    ));
                }

                "forge" => {
                    generator = Box::new(ForgeGenerator::new(
                        Loader::Forge,
                        version.name.clone(),
                        version.minecraft_version.clone(),
                        version.loader_version.clone(),
                        self.download_server_base.clone(),
                        version.replace_download_urls,
                    ));
                }

                "neoforge" => {
                    generator = Box::new(ForgeGenerator::new(
                        Loader::Neoforge,
                        version.name.clone(),
                        version.minecraft_version.clone(),
                        version.loader_version.clone(),
                        self.download_server_base.clone(),
                        version.replace_download_urls,
                    ));
                }

                _ => {
                    error!("Unsupported loader name: {}", version.loader_name);
                    continue;
                }
            }
            let result = generator.generate(output_dir, work_dir).await?;
            let id = result.id;
            let extra_libs_paths = result.extra_libs_paths;

            let resources_url_base = if version.replace_download_urls {
                self.resources_url_base.clone()
            } else {
                info!(
                    "Not setting resources_url_base for version {}",
                    version.name
                );
                None
            };
            let extra_generator = ExtraMetadataGenerator::new(
                version.name.clone(),
                version.include.clone(),
                version.include_no_overwrite.clone(),
                version.include_from.clone(),
                resources_url_base,
                self.download_server_base.clone(),
                extra_libs_paths,
                version.auth_provider.clone(),
            );
            extra_generator.generate(output_dir, work_dir).await?;

            let version_info =
                get_version_info(output_dir, &id, &version.name, &self.download_server_base)
                    .await?;
            version_manifest.versions.push(version_info);

            if let Some(command) = &version.exec_after {
                exec_custom_command(&command).await?;
            }
        }

        let manifest_path = get_manifest_path(output_dir);
        save_local_version_manifest(&version_manifest, &manifest_path).await?;

        if let Some(command) = &self.exec_after_all {
            exec_custom_command(&command).await?;
        }
        Ok(())
    }
}
