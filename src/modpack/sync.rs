use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::Client;

use crate::lang::LangMessage;
use crate::progress::ProgressBar;
use crate::{files, progress};

use super::index::{self, ModpackIndex};
use super::{asset_metadata, version_metadata};

pub struct PathData {
    pub modpack_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub index_path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum VersionMetadataError {
    #[error("Library {0} has neither SHA1 hash nor SHA1 URL")]
    NoSha1(String),
    #[error("Library {0} has no download URL")]
    NoUrl(String),
}

async fn download_assets_and_libraries(
    asset_index: &version_metadata::AssetIndex,
    assets_dir: &Path,
    resources_url_base: &str,
    libraries_dir: &Path,
    libraries: &Vec<version_metadata::Library>,
    progress_bar: Arc<dyn ProgressBar + Send + Sync>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut urls: Vec<String> = vec![];
    let mut paths: Vec<PathBuf> = vec![];

    progress_bar.set_message(LangMessage::CheckingAssets);

    let asset_index_path = assets_dir.join("indexes").join(&asset_index.id);
    let need_assets_download: bool;
    if !asset_index_path.exists() {
        need_assets_download = true;
    } else {
        let local_asset_index_hash = files::hash_file(&asset_index_path).await?;
        need_assets_download = local_asset_index_hash != asset_index.sha1;
    }

    if need_assets_download {
        let client = Client::new();
        files::download_file(&client, &asset_index.url, &asset_index_path).await?;
        let asset_metadata = asset_metadata::read_asset_metadata(&asset_index_path).await?;

        for (hash, object) in asset_metadata.objects.iter() {
            let object_path = assets_dir.join("objects").join(&hash[..2]).join(hash);
            let need_download: bool;
            if !object_path.exists() {
                need_download = true;
            } else {
                let local_object_hash = files::hash_file(&object_path).await?;
                need_download = local_object_hash != object.hash;
            }

            if need_download {
                urls.push(format!(
                    "{}/objects/{}/{}",
                    resources_url_base,
                    &hash[..2],
                    hash
                ));
                paths.push(object_path);
            }
        }
    }

    let existing_libraries_paths: Vec<PathBuf> = libraries
        .iter()
        .map(|x| libraries_dir.join(x.get_path()))
        .filter(|x| x.exists())
        .collect();
    let libraries_hashes = files::hash_files(
        existing_libraries_paths.clone().into_iter(),
        progress_bar.clone(),
    )
    .await?;
    let libraries_hashes: HashMap<PathBuf, String> = existing_libraries_paths
        .into_iter()
        .zip(libraries_hashes.into_iter())
        .collect();

    let mut remote_libraries_hashes: HashMap<PathBuf, String> = libraries
        .iter()
        .filter_map(|x| {
            x.get_sha1()
                .map(|url| (libraries_dir.join(x.get_path()), url))
        })
        .collect();

    let existing_libraries_with_missing_hashes: Vec<&version_metadata::Library> = libraries
        .iter()
        .filter(|x| {
            let path = libraries_dir.join(x.get_path());
            !remote_libraries_hashes.contains_key(&path) && !libraries_hashes.contains_key(&path)
        })
        .collect();
    if !existing_libraries_with_missing_hashes.is_empty() {
        let sha1_urls = existing_libraries_with_missing_hashes
            .iter()
            .map(|x| {
                x.get_sha1_url()
                    .ok_or_else(|| VersionMetadataError::NoSha1(x.name.clone()))
            })
            .collect::<Result<Vec<String>, _>>()?;
        let fetched_hashes = files::fetch_files(sha1_urls.into_iter(), progress::no_progress_bar())
            .await?
            .into_iter()
            .map(|x| String::from_utf8(x))
            .collect::<Result<Vec<String>, _>>()?;

        remote_libraries_hashes.extend(
            existing_libraries_with_missing_hashes
                .iter()
                .map(|x| libraries_dir.join(x.get_path()))
                .zip(fetched_hashes.into_iter()),
        );
    }

    for library in libraries.iter() {
        let library_path = libraries_dir.join(library.get_path());
        let need_download: bool;
        match libraries_hashes.get(&library_path) {
            Some(hash) => need_download = hash != &remote_libraries_hashes[&library_path],
            None => need_download = true,
        }
        if need_download {
            urls.push(
                library
                    .get_url()
                    .ok_or(VersionMetadataError::NoUrl(library.name.clone()))?,
            );
            paths.push(library_path);
        }
    }

    if !urls.is_empty() {
        progress_bar.set_message(LangMessage::DownloadingAssets);
        files::download_files(urls.into_iter(), paths.into_iter(), progress_bar.clone()).await?;
    }
    Ok(())
}

pub async fn sync_modpack(
    index: &ModpackIndex,
    force_overwrite: bool,
    path_data: PathData,
    progress_bar: Arc<dyn ProgressBar + Send + Sync>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let PathData {
        modpack_dir,
        assets_dir,
        index_path,
    } = path_data;

    let get_modpack_files = |x| files::get_files_in_dir(&modpack_dir.join(x));
    let no_overwrite_iter = index
        .include_no_overwrite
        .iter()
        .map(get_modpack_files)
        .flatten();
    let mut abs_path_overwrite: HashSet<PathBuf> = index
        .include
        .iter()
        .map(get_modpack_files)
        .flatten()
        .collect();
    let mut abs_path_no_overwrite = HashSet::new();
    if !force_overwrite {
        abs_path_no_overwrite.extend(no_overwrite_iter);
    } else {
        abs_path_overwrite.extend(no_overwrite_iter);
    }

    // Remove files that are in both no_overwrite and overwrite
    // e.g. config folder is in no_overwrite but config/<filename>.json is in overwrite
    abs_path_no_overwrite.retain(|x| !abs_path_overwrite.contains(x));

    let abs_path_overwrite_hashes =
        files::hash_files(abs_path_overwrite.clone().into_iter(), progress_bar.clone()).await?;
    let abs_path_overwrite_hashes: HashMap<PathBuf, String> = abs_path_overwrite
        .clone()
        .into_iter()
        .zip(abs_path_overwrite_hashes.into_iter())
        .collect();

    let mut urls: Vec<String> = vec![];
    let mut paths: Vec<PathBuf> = vec![];

    let objects_filepaths: HashSet<PathBuf> = index
        .objects
        .iter()
        .map(|x| modpack_dir.join(&x.path))
        .collect();

    for path in abs_path_overwrite.iter() {
        if !objects_filepaths.contains(path) {
            fs::remove_file(path)?;
        }
    }
    for object in index.objects.iter() {
        let object_path = modpack_dir.join(&object.path);

        if abs_path_no_overwrite.contains(&object_path) {
            continue;
        }
        let need_download: bool;
        match abs_path_overwrite_hashes.get(&object_path) {
            Some(hash) => need_download = hash != &object.sha1,
            None => need_download = true,
        }
        if need_download {
            urls.push(object.url.clone());
            paths.push(object_path);
        }
    }

    progress_bar.set_message(LangMessage::DownloadingModpackFiles);
    files::download_files(urls.into_iter(), paths.into_iter(), progress_bar.clone()).await?;

    let version_dir = modpack_dir.join("version");
    let merged_version_metadata = version_metadata::get_merged_metadata(&version_dir).await?;

    let asset_index = &merged_version_metadata.asset_index;
    let libraries_dir = modpack_dir.join("libraries");
    let libraries = &merged_version_metadata.libraries;

    download_assets_and_libraries(
        &asset_index,
        &assets_dir,
        index.get_resources_url_base(),
        &libraries_dir,
        libraries,
        progress_bar,
    )
    .await?;

    index::save_local_index(&index_path, index);
    Ok(())
}
