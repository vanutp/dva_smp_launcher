use std::{
    error::Error,
    path::{Path, PathBuf}, sync::Arc,
};

use fs_extra::{dir, file};
use log::{error, info};
use shared::{
    files::{self, get_files_in_dir, hash_files},
    paths::{get_libraries_dir, get_minecraft_dir, get_versions_extra_dir},
    progress::{self, ProgressBar as _},
    version::{
        extra_version_metadata::{save_extra_version_metadata, ExtraVersionMetadata, Object},
        version_metadata::Library,
    },
};

use crate::{progress::TerminalProgressBar, utils::get_url_from_path};

#[derive(thiserror::Error, Debug)]
pub enum ExtraMetadataGeneratorError {
    #[error("Path does not exist: {0}")]
    PathDoesNotExist(String),
}

fn sync_paths(from: &Path, to: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !from.exists() {
        return Err(Box::new(ExtraMetadataGeneratorError::PathDoesNotExist(
            from.to_string_lossy().to_string(),
        )));
    }

    if from.is_file() && to.is_dir() {
        // scary
        std::fs::remove_dir_all(to)?;
    }
    if from.is_dir() && to.is_file() {
        std::fs::remove_file(to)?;
    }

    if from.is_file() {
        let mut options = file::CopyOptions::new();
        options.overwrite = true;
        options.skip_exist = true;

        file::copy(from, to, &options)?;
    } else {
        let mut options = dir::CopyOptions::new();
        options.overwrite = true;
        options.skip_exist = true;

        dir::copy(from, to.parent().unwrap(), &options)?;
    }

    Ok(())
}

async fn get_objects(
    path: &Path,
    data_dir: &Path,
    version_name: &str,
    download_server_base: &str,
) -> Result<Vec<Object>, Box<dyn Error + Send + Sync>> {
    let minecraft_dir = get_minecraft_dir(data_dir, version_name);

    let paths = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        get_files_in_dir(path)
    };

    let hashes = files::hash_files(paths.clone(), progress::no_progress_bar()).await?;

    let mut objects = vec![];
    for (path, hash) in paths.iter().zip(hashes.iter()) {
        let url = get_url_from_path(path, data_dir, download_server_base)?;
        objects.push(Object {
            path: path
                .strip_prefix(&minecraft_dir)?
                .to_string_lossy()
                .to_string(),
            sha1: hash.clone(),
            url,
        });
    }

    Ok(objects)
}

#[derive(thiserror::Error, Debug)]
pub enum ExtraForgeLibsError {
    #[error("Bad library name: {0}")]
    BadLibraryName(String),
}

async fn get_extra_forge_libs(
    extra_forge_libs_paths: &Vec<PathBuf>,
    data_dir: &Path,
    version_name: &str,
    download_server_base: &str,
) -> Result<Vec<Library>, Box<dyn Error + Send + Sync>> {
    let libraries_dir = get_libraries_dir(data_dir, version_name);

    let progress_bar = Arc::new(TerminalProgressBar::new());
    progress_bar.set_message("Hashing extra forge libraries");
    let hashes = hash_files(extra_forge_libs_paths.to_vec(), progress_bar).await?;

    let libraries = extra_forge_libs_paths
        .iter()
        .zip(hashes.iter())
        .filter(|(path, _)| path.is_file() && path.extension().map_or(false, |ext| ext == "jar"))
        .map(|(path, hash)| {
            let url = get_url_from_path(path, data_dir, download_server_base)?;

            let parts = path.strip_prefix(&libraries_dir)?.components().map(|x| x.as_os_str().to_string_lossy()).collect::<Vec<_>>();
            let version = parts[parts.len() - 2].to_string();
            let name = parts[parts.len() - 3].to_string();
            let group = parts.iter().take(parts.len() - 3).map(|s| s.to_string()).collect::<Vec<_>>().join(".");

            let filename = path.file_name().unwrap().to_string_lossy().strip_suffix(".jar").unwrap().to_string();
            let filename_without_suffix = format!("{}-{}", name, version);
            let suffix = filename.strip_prefix(&filename_without_suffix).ok_or(ExtraForgeLibsError::BadLibraryName(filename.clone()))?;
            let suffix = suffix.replace("-", ":");

            let name = format!("{}:{}:{}{}", group, name, version, suffix);

            Ok(Library::from_download(name, url, hash.clone()))
        })
        .collect::<Result<_, Box<dyn Error + Send + Sync>>>()?;

    Ok(libraries)
}

pub struct ExtraMetadataGenerator {
    version_name: String,
    include: Vec<String>,
    include_no_overwrite: Vec<String>,
    include_from: Option<String>,
    resources_url_base: Option<String>,
    download_server_base: String,
    extra_forge_libs_paths: Vec<PathBuf>,
}

impl ExtraMetadataGenerator {
    pub fn new(
        version_name: String,
        include: Vec<String>,
        include_no_overwrite: Vec<String>,
        include_from: Option<String>,
        resources_url_base: Option<String>,
        download_server_base: String,
        extra_forge_libs_paths: Vec<PathBuf>,
    ) -> Self {
        Self {
            version_name,
            include,
            include_no_overwrite,
            include_from,
            resources_url_base,
            download_server_base,
            extra_forge_libs_paths,
        }
    }

    pub async fn generate(&self, output_dir: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!(
            "Generating extra metadata for modpack {}",
            self.version_name
        );

        if self.include_from.is_none() && self.resources_url_base.is_none() {
            return Ok(());
        }

        let extra_forge_libs = get_extra_forge_libs(
            &self.extra_forge_libs_paths,
            output_dir,
            &self.version_name,
            &self.download_server_base,
        ).await?;

        let mut extra_metadata = ExtraVersionMetadata {
            version_name: self.version_name.clone(),
            include: self.include.clone(),
            include_no_overwrite: self.include_no_overwrite.clone(),
            objects: Vec::new(),
            resources_url_base: self.resources_url_base.clone(),
            extra_forge_libs,
        };

        if let Some(include_from) = &self.include_from {
            let copy_from = PathBuf::from(include_from);
            let copy_to = get_minecraft_dir(output_dir, &self.version_name);

            let mut objects = vec![];
            for include in self.include.iter().chain(self.include_no_overwrite.iter()) {
                let from = copy_from.join(include);
                let to = copy_to.join(include);

                info!(
                    "Copying {} from {} to {}",
                    include,
                    from.to_string_lossy(),
                    to.to_string_lossy()
                );
                sync_paths(&from, &to)?;

                objects.extend(
                    get_objects(
                        &to,
                        output_dir,
                        &self.version_name,
                        &self.download_server_base,
                    )
                    .await?,
                );
            }

            extra_metadata.objects = objects;
        }

        let versions_extra_dir = get_versions_extra_dir(output_dir);
        save_extra_version_metadata(&versions_extra_dir, &self.version_name, &extra_metadata)
            .await?;

        info!("Extra metadata for modpack {} generated", self.version_name);

        Ok(())
    }
}
