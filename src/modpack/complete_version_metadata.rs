use std::path::Path;

use super::{
    extra_version_metadata::{
        get_extra_version_metadata, read_local_extra_version_metadata, ExtraVersionMetadata,
    },
    version_manifest::VersionInfo,
    version_metadata::{
        get_merged_version_metadata, read_local_merged_version_metadata, MergedVersionMetadata,
    },
};

pub struct CompleteVersionMetadata {
    pub base: MergedVersionMetadata,
    pub extra: Option<ExtraVersionMetadata>,
}

const DEFAULT_RESOURCES_URL_BASE: &str = "https://resources.download.minecraft.net";

impl CompleteVersionMetadata {
    pub fn get_resources_url_base(&self) -> &str {
        if let Some(extra) = &self.extra {
            return extra
                .resources_url_base
                .as_ref()
                .map(|x| x.as_str())
                .unwrap_or(DEFAULT_RESOURCES_URL_BASE);
        } else {
            return DEFAULT_RESOURCES_URL_BASE;
        }
    }

    pub fn get_java_version(&self) -> String {
        return self.base.java_version.major_version.to_string();
    }
}

pub async fn get_complete_version_metadata(
    version_info: &VersionInfo,
    versions_dir: &Path,
) -> Result<CompleteVersionMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let base = get_merged_version_metadata(version_info, versions_dir).await?;
    let extra = get_extra_version_metadata(version_info, versions_dir).await?;
    Ok(CompleteVersionMetadata { base, extra })
}

pub async fn read_local_complete_version_metadata(
    version_info: &VersionInfo,
    versions_dir: &Path,
) -> Result<CompleteVersionMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let base = read_local_merged_version_metadata(&version_info.id, versions_dir).await?;
    let extra = read_local_extra_version_metadata(version_info, versions_dir).await?;
    Ok(CompleteVersionMetadata { base, extra })
}
