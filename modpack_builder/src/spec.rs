use log::{debug, error, info, warn};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use tokio::fs;

use shared::{
    files::sync_mapping,
    paths::{
        get_extra_metadata_path, get_instance_dir, get_manifest_path, get_metadata_path,
        get_versions_dir, get_versions_extra_dir,
    },
    utils::BoxResult,
    version::{
        asset_metadata::AssetsMetadata, extra_version_metadata::AuthData,
        version_manifest::VersionManifest,
    },
};

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
        mapping::get_mapping,
        patch::replace_download_urls,
        sync::sync_version,
    },
    utils::{
        exec_custom_command, get_assets_dir, get_replaced_metadata_dir, get_vanilla_version_info,
        VANILLA_MANIFEST_URL,
    },
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
    pub auth_provider: AuthData,

    pub exec_before: Option<String>,
    pub exec_after: Option<String>,
}

#[derive(Deserialize)]
pub struct VersionsSpec {
    pub download_server_base: String,
    pub resources_url_base: Option<String>,

    #[serde(default)]
    pub replace_download_urls: bool,

    pub versions: Vec<Version>,
    pub exec_before_all: Option<String>,
    pub exec_after_all: Option<String>,
}

impl VersionsSpec {
    pub async fn from_file(path: &Path) -> BoxResult<VersionsSpec> {
        let content = fs::read_to_string(path).await?;
        let spec = serde_json::from_str(&content)?;
        Ok(spec)
    }

    pub async fn generate(&self, output_dir: &Path, work_dir: &Path) -> BoxResult<()> {
        if let Some(command) = &self.exec_before_all {
            exec_custom_command(&command).await?;
        }

        info!("Fetching version manifest");
        let vanilla_manifest = VersionManifest::fetch(VANILLA_MANIFEST_URL).await?;

        let mut version_manifest = VersionManifest { versions: vec![] };
        let mut mapping = HashMap::new();

        for version in &self.versions {
            if let Some(command) = &version.exec_before {
                exec_custom_command(&command).await?;
            }

            let vanilla_version_info =
                get_vanilla_version_info(&vanilla_manifest, &version.minecraft_version)?;

            let generator: Box<dyn VersionGenerator>;
            match version.loader_name.as_str() {
                "vanilla" => {
                    if version.loader_version.is_some() {
                        warn!("Ignoring loader version for vanilla version");
                    }

                    generator = Box::new(VanillaGenerator::new(
                        version.name.clone(),
                        vanilla_version_info,
                    ));
                }

                "fabric" => {
                    generator = Box::new(FabricGenerator::new(
                        version.name.clone(),
                        vanilla_version_info,
                        version.loader_version.clone(),
                    ));
                }

                "forge" => {
                    generator = Box::new(ForgeGenerator::new(
                        version.name.clone(),
                        vanilla_version_info,
                        Loader::Forge,
                        version.loader_version.clone(),
                    ));
                }

                "neoforge" => {
                    generator = Box::new(ForgeGenerator::new(
                        version.name.clone(),
                        vanilla_version_info,
                        Loader::Neoforge,
                        version.loader_version.clone(),
                    ));
                }

                _ => {
                    error!("Unsupported loader name: {}", version.loader_name);
                    continue;
                }
            }

            let mut workdir_paths_to_copy = vec![];

            let mut result = generator.generate(work_dir).await?;
            if self.replace_download_urls {
                let versions_dir = get_versions_dir(output_dir);
                let replaced_metadata_dir = get_replaced_metadata_dir(work_dir);

                for metadata in result.metadata.iter_mut() {
                    info!("Syncing {}", &metadata.id);

                    let sync_result = sync_version(metadata, work_dir).await?;
                    if let Some(asset_index) = &metadata.asset_index {
                        let assets_dir = get_assets_dir(work_dir);
                        let asset_index_path =
                            AssetsMetadata::get_path(&assets_dir, &asset_index.id).await?;
                        workdir_paths_to_copy.push(asset_index_path);
                    }
                    workdir_paths_to_copy.extend(sync_result.paths_to_copy);

                    replace_download_urls(metadata, &self.download_server_base, work_dir).await?;
                    metadata.save(&replaced_metadata_dir).await?;

                    mapping.insert(
                        get_metadata_path(&versions_dir, &metadata.id),
                        get_metadata_path(&replaced_metadata_dir, &metadata.id),
                    );
                }
            } else {
                let versions_dir = get_versions_dir(output_dir);
                for metadata in result.metadata.iter_mut() {
                    workdir_paths_to_copy.push(get_metadata_path(&versions_dir, &metadata.id));
                }
            }
            workdir_paths_to_copy.extend(result.extra_libs_paths.clone());

            let resources_url_base = if self.replace_download_urls {
                self.resources_url_base.clone()
            } else {
                None
            };
            let extra_generator = ExtraMetadataGenerator::new(
                version.name.clone(),
                version.include.clone(),
                version.include_no_overwrite.clone(),
                version.include_from.clone(),
                resources_url_base,
                self.download_server_base.clone(),
                result.extra_libs_paths,
                version.auth_provider.clone(),
            );
            let extra_generator_result = extra_generator.generate(work_dir).await?;
            workdir_paths_to_copy.extend(extra_generator_result.paths_to_copy);
            mapping.extend(extra_generator_result.include_mapping.into_iter().map(
                |(include_entry, source_path)| {
                    let instance_dir = get_instance_dir(output_dir, &version.name);
                    (instance_dir.join(include_entry), source_path)
                },
            ));

            let versions_extra_dir = get_versions_extra_dir(work_dir);
            workdir_paths_to_copy.push(get_extra_metadata_path(&versions_extra_dir, &version.name));

            info!("Getting version info for {}", &version.name);
            let version_info = get_version_info(
                work_dir,
                &result.metadata,
                &version.name,
                &self.download_server_base,
            )
            .await?;
            version_manifest.versions.push(version_info);

            mapping.extend(get_mapping(output_dir, work_dir, &workdir_paths_to_copy)?);

            if let Some(command) = &version.exec_after {
                exec_custom_command(&command).await?;
            }

            info!("Finished generating version {}", &version.name);
        }

        info!("Copying {} files to output directory", mapping.len());
        debug!("Paths to copy: {:?}", mapping);
        sync_mapping(output_dir, &mapping).await?;

        let manifest_path = get_manifest_path(output_dir);
        version_manifest.save_to_file(&manifest_path).await?;

        if let Some(command) = &self.exec_after_all {
            exec_custom_command(&command).await?;
        }
        Ok(())
    }
}
