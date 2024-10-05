use std::{path::Path, sync::Arc};

use shared::{files::{download_files, get_download_entries, CheckDownloadEntry}, paths::get_libraries_dir, progress::ProgressBar, version::{asset_metadata::AssetsMetadata, version_metadata::VersionMetadata}};

use crate::{progress::TerminalProgressBar, utils::get_assets_dir};

pub fn get_libraries_check_downloads(
    version_metadata: &VersionMetadata,
    libraries_dir: &Path,
) -> Vec<CheckDownloadEntry> {
    let mut entries = vec![];
    for library in &version_metadata.libraries {
        entries.extend(library.get_all_check_download_entries(libraries_dir));
    }
    entries
}

const RESOURCES_URL_BASE: &str = "https://resources.download.minecraft.net";

pub async fn sync_version(
    version_metadata: &VersionMetadata,
    version_name: &str,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let libraries_dir = get_libraries_dir(output_dir, version_name);
    let mut check_entries = get_libraries_check_downloads(version_metadata, &libraries_dir);
    
    if let Some(asset_index) = &version_metadata.asset_index {
        let assets_dir = get_assets_dir(output_dir);
        let assets_metadata = AssetsMetadata::read_or_fetch(asset_index, &assets_dir).await?;
        let asset_check_entries = assets_metadata.get_check_downloads(&assets_dir, RESOURCES_URL_BASE)?;
        check_entries.extend(asset_check_entries);
    }

    let progress_bar = Arc::new(TerminalProgressBar::new());

    progress_bar.set_message("Checking files...");
    let download_entries = get_download_entries(check_entries, progress_bar.clone()).await?;

    progress_bar.set_message("Downloading files...");
    download_files(download_entries, progress_bar).await?;

    Ok(())
}
