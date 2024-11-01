use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{files::CheckEntry, paths::get_extra_metadata_path, utils::BoxResult};

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
pub struct MicrosoftAuthData {
    pub client_id: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthData {
    None,
    Telegram(TelegramAuthData),
    #[serde(rename = "ely.by")]
    ElyBy(ElyByAuthData),
    Microsoft(MicrosoftAuthData),
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
            AuthData::ElyBy(auth_data) => {
                format!("elyby_{}_{}", auth_data.client_id, auth_data.client_secret)
            }
            AuthData::Microsoft(auth_data) => format!("microsoft_{}", auth_data.client_id),
            AuthData::None => "none".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct ExtraVersionMetadata {
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
    pub authlib_injector: Option<Object>,

    #[serde(default)]
    pub extra_forge_libs: Vec<Library>,
}

impl ExtraVersionMetadata {
    pub async fn read_local(
        version_info: &VersionInfo,
        versions_extra_dir: &Path,
    ) -> BoxResult<Option<Self>> {
        if version_info.extra_metadata_url.is_none() || version_info.extra_metadata_sha1.is_none() {
            return Ok(None);
        }

        let extra_version_metadata_path =
            get_extra_metadata_path(versions_extra_dir, &version_info.get_name());
        let extra_version_metadata_file = tokio::fs::read(extra_version_metadata_path).await?;

        Ok(Some(serde_json::from_slice(&extra_version_metadata_file)?))
    }

    pub fn get_check_entry(
        version_info: &VersionInfo,
        versions_extra_dir: &Path,
    ) -> Option<CheckEntry> {
        if version_info.extra_metadata_url.is_none() || version_info.extra_metadata_sha1.is_none() {
            return None;
        }

        let url = version_info.extra_metadata_url.as_ref().unwrap();
        let sha1 = version_info.extra_metadata_sha1.as_ref().unwrap();

        Some(CheckEntry {
            url: url.clone(),
            remote_sha1: Some(sha1.clone()),
            path: get_extra_metadata_path(versions_extra_dir, &version_info.get_name()),
        })
    }

    pub async fn save(&self, version_name: &str, versions_extra_dir: &Path) -> BoxResult<()> {
        let path = get_extra_metadata_path(versions_extra_dir, version_name);
        let serialized = serde_json::to_string(self)?;
        tokio::fs::write(path, serialized).await?;

        Ok(())
    }
}
