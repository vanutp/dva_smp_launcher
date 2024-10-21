use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files;

use super::{version_manifest::VersionInfo, version_metadata::Library};

#[derive(Deserialize, Serialize, Debug)]
pub struct Object {
    pub path: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TelegramAuthData {
    pub auth_base_url: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ElyByAuthData {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthData {
    None,
    Telegram(TelegramAuthData),
    #[serde(rename = "ely.by")]
    ElyBy(ElyByAuthData),
}

impl Default for AuthData {
    fn default() -> Self {
        AuthData::None
    }
}

impl AuthData {
    pub fn get_id(&self) -> String {
        match self {
            AuthData::Telegram(auth_data) => format!("telegram_{}", auth_data.auth_base_url),
            AuthData::ElyBy(auth_data) => format!(
                "elyby_{}_{}",
                auth_data.client_id, auth_data.client_secret
            ),
            AuthData::None => "none".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct ExtraVersionMetadata {
    pub version_name: String,

    pub auth_provider: AuthData,

    #[serde(default)]
    pub include: Vec<String>,

    #[serde(default)]
    pub include_no_overwrite: Vec<String>,

    #[serde(default)]
    pub objects: Vec<Object>,

    #[serde(default)]
    pub resources_url_base: Option<String>,

    #[serde(default)]
    pub extra_forge_libs: Vec<Library>,
}

pub fn get_extra_version_metadata_path(versions_extra_dir: &Path, version_name: &str) -> PathBuf {
    versions_extra_dir.join(format!("{}.json", version_name))
}

pub async fn read_local_extra_version_metadata(
    version_info: &VersionInfo,
    versions_extra_dir: &Path,
) -> Result<Option<ExtraVersionMetadata>, Box<dyn std::error::Error + Send + Sync>> {
    if version_info.extra_metadata_url.is_none() || version_info.extra_metadata_sha1.is_none() {
        return Ok(None);
    }

    let extra_version_metadata_path =
        get_extra_version_metadata_path(versions_extra_dir, &version_info.get_name());
    let extra_version_metadata_file = tokio::fs::read(extra_version_metadata_path).await?;
    let extra_version_metadata: ExtraVersionMetadata =
        serde_json::from_slice(&extra_version_metadata_file)?;

    Ok(Some(extra_version_metadata))
}

pub async fn get_extra_version_metadata(
    version_info: &VersionInfo,
    versions_extra_dir: &Path,
) -> Result<Option<ExtraVersionMetadata>, Box<dyn std::error::Error + Send + Sync>> {
    if version_info.extra_metadata_url.is_none() || version_info.extra_metadata_sha1.is_none() {
        return Ok(None);
    }

    let url = version_info.extra_metadata_url.as_ref().unwrap();
    let sha1 = version_info.extra_metadata_sha1.as_ref().unwrap();

    let extra_version_metadata_path =
        get_extra_version_metadata_path(versions_extra_dir, &version_info.get_name());
    let need_download;
    if !extra_version_metadata_path.exists() {
        need_download = true;
    } else {
        let local_sha1 = files::hash_file(&extra_version_metadata_path).await?;
        need_download = &local_sha1 != sha1;
    }

    if need_download {
        let client = reqwest::Client::new();
        files::download_file(&client, url, &extra_version_metadata_path).await?;
    }

    Ok(read_local_extra_version_metadata(version_info, versions_extra_dir).await?)
}

pub async fn save_extra_version_metadata(
    versions_extra_dir: &Path,
    version_name: &str,
    extra_version_metadata: &ExtraVersionMetadata,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let extra_version_metadata_path =
        get_extra_version_metadata_path(versions_extra_dir, version_name);
    let extra_version_metadata_file = serde_json::to_string(extra_version_metadata)?;
    tokio::fs::write(extra_version_metadata_path, extra_version_metadata_file).await?;

    Ok(())
}
