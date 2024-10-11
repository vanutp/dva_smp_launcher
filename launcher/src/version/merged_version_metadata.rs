use std::{error::Error, path::Path};

use shared::{
    files::{self, CheckDownloadEntry},
    progress,
    version::{
        version_manifest::VersionInfo,
        version_metadata::{
            get_version_metadata_path, read_version_metadata, Arguments, AssetIndex, Downloads,
            JavaVersion, Library, VersionMetadata,
        },
    },
};

#[derive(thiserror::Error, Debug)]
pub enum VersionMetadataError {
    #[error("Bad version metadata")]
    BadMetadata,
}

pub struct MergedVersionMetadata {
    pub arguments: Arguments,
    pub asset_index: AssetIndex,
    pub id: String,
    pub java_version: JavaVersion,
    libraries: Vec<Library>,
    pub main_class: String,
    pub downloads: Option<Downloads>,
    pub hierarchy_ids: Vec<String>,
}

impl MergedVersionMetadata {
    fn from_version_metadata(
        version_metadata: VersionMetadata,
    ) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
        Ok(MergedVersionMetadata {
            arguments: version_metadata.get_arguments()?,
            asset_index: version_metadata
                .asset_index
                .ok_or(Box::new(VersionMetadataError::BadMetadata))?,
            id: version_metadata.id.clone(),
            java_version: version_metadata
                .java_version
                .ok_or(Box::new(VersionMetadataError::BadMetadata))?,
            libraries: version_metadata.libraries,
            main_class: version_metadata.main_class,
            downloads: version_metadata.downloads,
            hierarchy_ids: vec![version_metadata.id],
        })
    }

    pub fn get_libraries(&self) -> Vec<&Library> {
        self.libraries.iter().collect()
    }
}

fn merge_two_metadata(
    parent_metadata: &mut MergedVersionMetadata,
    child_metadata: VersionMetadata,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(arguments) = child_metadata.arguments {
        parent_metadata.arguments.game.extend(arguments.game);
        parent_metadata.arguments.jvm.extend(arguments.jvm);
    } else if child_metadata.minecraft_arguments.is_some() {
        let arguments = child_metadata.get_arguments()?;
        parent_metadata.arguments.game = arguments.game;
    }

    let parent_id = parent_metadata.id.clone();
    parent_metadata.hierarchy_ids.push(parent_id);
    parent_metadata.id = child_metadata.id.clone();

    if let Some(java_version) = child_metadata.java_version {
        parent_metadata.java_version = java_version;
    }

    parent_metadata.libraries.extend(child_metadata.libraries);

    parent_metadata.main_class = child_metadata.main_class;

    if parent_metadata.downloads.is_none() && child_metadata.downloads.is_some() {
        parent_metadata.downloads = child_metadata.downloads;
    }

    Ok(())
}

pub async fn read_local_merged_version_metadata(
    version_id: &str,
    versions_dir: &Path,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let mut metadata = vec![];
    let mut version_id = version_id.to_string();
    loop {
        let current_metadata = read_version_metadata(versions_dir, &version_id).await?;
        let parent_id = current_metadata.inherits_from.clone();
        metadata.push(current_metadata);
        if let Some(id) = parent_id {
            version_id = id;
        } else {
            break;
        }
    }

    let mut merged_metadata =
        MergedVersionMetadata::from_version_metadata(metadata.pop().unwrap())?;
    while let Some(current_metadata) = metadata.pop() {
        merge_two_metadata(&mut merged_metadata, current_metadata)?;
    }

    Ok(merged_metadata)
}

pub async fn get_merged_version_metadata(
    version_info: &VersionInfo,
    versions_dir: &Path,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let metadata_info = version_info.get_metadata_info();

    let check_entries: Vec<CheckDownloadEntry> = metadata_info
        .iter()
        .map(|metadata| CheckDownloadEntry {
            url: metadata.url.clone(),
            remote_sha1: Some(metadata.sha1.clone()),
            path: get_version_metadata_path(versions_dir, &metadata.id),
        })
        .collect();

    let download_entries =
        files::get_download_entries(check_entries, progress::no_progress_bar()).await?;
    files::download_files(download_entries, progress::no_progress_bar()).await?;

    read_local_merged_version_metadata(&version_info.id, versions_dir).await
}
